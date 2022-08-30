//! Generators may generate bytes or, in general, data, for inputs.

use alloc::vec::Vec;
use core::{cmp::min, marker::PhantomData};

use crate::{
    bolts::rands::Rand,
    inputs::{bytes::BytesInput, GeneralizedInput, Input},
    state::HasRand,
    Error,
};

pub mod gramatron;
pub use gramatron::*;

#[cfg(feature = "nautilus")]
pub mod nautilus;
#[cfg(feature = "nautilus")]
pub use nautilus::*;

/// The maximum size of dummy bytes generated by _dummy generator methods
const DUMMY_BYTES_MAX: usize = 64;

/// Generators can generate ranges of bytes.
pub trait Generator {
    type Input: Input;
    type State;

    /// Generate a new input
    fn generate(&mut self, state: &mut Self::State) -> Result<Self::Input, Error>;

    /// Generate a new dummy input
    fn generate_dummy(&self, state: &mut Self::State) -> Self::Input;
}

/// A Generator that produces [`GeneralizedInput`]s from a wrapped [`BytesInput`] generator
#[derive(Clone, Debug)]
pub struct GeneralizedInputBytesGenerator<G, S> {
    bytes_generator: G,
    phantom: PhantomData<S>,
}

impl<G, S> GeneralizedInputBytesGenerator<G, S>
where
    S: HasRand,
    G: Generator<Input = BytesInput, State = S>,
{
    /// Creates a new [`GeneralizedInputBytesGenerator`] by wrapping a bytes generator.
    pub fn new(bytes_generator: G) -> Self {
        Self {
            bytes_generator,
            phantom: PhantomData,
        }
    }
}

impl<G, S> From<G> for GeneralizedInputBytesGenerator<G, S>
where
    S: HasRand,
    G: Generator<Input = BytesInput, State = S>,
{
    fn from(bytes_generator: G) -> Self {
        Self::new(bytes_generator)
    }
}

impl<G, S> Generator for GeneralizedInputBytesGenerator<G, S>
where
    S: HasRand,
    G: Generator<Input = BytesInput, State = S>,
{
    fn generate(&mut self, state: &mut S) -> Result<GeneralizedInput, Error> {
        Ok(self.bytes_generator.generate(state)?.into())
    }

    fn generate_dummy(&self, state: &mut S) -> GeneralizedInput {
        self.bytes_generator.generate_dummy(state).into()
    }
}

#[derive(Clone, Debug)]
/// Generates random bytes
pub struct RandBytesGenerator {
    max_size: usize,
}

impl Generator for RandBytesGenerator {
    type Input = BytesInput;

    fn generate(&mut self, state: &mut Self::State) -> Result<BytesInput, Error> {
        let mut size = state.rand_mut().below(self.max_size as u64);
        if size == 0 {
            size = 1;
        }
        let random_bytes: Vec<u8> = (0..size)
            .map(|_| state.rand_mut().below(256) as u8)
            .collect();
        Ok(BytesInput::new(random_bytes))
    }

    /// Generates up to `DUMMY_BYTES_MAX` non-random dummy bytes (0)
    fn generate_dummy(&self, _state: &mut Self::State) -> BytesInput {
        let size = min(self.max_size, DUMMY_BYTES_MAX);
        BytesInput::new(vec![0; size])
    }
}

impl RandBytesGenerator {
    /// Returns a new [`RandBytesGenerator`], generating up to `max_size` random bytes.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }
}

#[derive(Clone, Debug)]
/// Generates random printable characters
pub struct RandPrintablesGenerator {
    max_size: usize,
}

impl Generator for RandPrintablesGenerator {
    type Input = BytesInput;

    fn generate(&mut self, state: &mut Self::State) -> Result<BytesInput, Error> {
        let mut size = state.rand_mut().below(self.max_size as u64);
        if size == 0 {
            size = 1;
        }
        let printables = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz \t\n!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".as_bytes();
        let random_bytes: Vec<u8> = (0..size)
            .map(|_| *state.rand_mut().choose(printables))
            .collect();
        Ok(BytesInput::new(random_bytes))
    }

    /// Generates up to `DUMMY_BYTES_MAX` non-random dummy bytes (0)
    fn generate_dummy(&self, _state: &mut Self::State) -> BytesInput {
        let size = min(self.max_size, DUMMY_BYTES_MAX);
        BytesInput::new(vec![0_u8; size])
    }
}

impl RandPrintablesGenerator {
    /// Creates a new [`RandPrintablesGenerator`], generating up to `max_size` random printable characters.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }
}

/// `Generator` Python bindings
#[allow(missing_docs)]
#[cfg(feature = "python")]
pub mod pybind {
    use alloc::vec::Vec;

    use pyo3::prelude::*;

    use crate::{
        generators::{Generator, RandBytesGenerator, RandPrintablesGenerator},
        inputs::{BytesInput, HasBytesVec},
        state::pybind::{PythonStdState, PythonStdStateWrapper},
        Error,
    };

    #[derive(Clone, Debug)]
    pub struct PyObjectGenerator {
        inner: PyObject,
    }

    impl PyObjectGenerator {
        #[must_use]
        pub fn new(obj: PyObject) -> Self {
            PyObjectGenerator { inner: obj }
        }
    }

    impl Generator<BytesInput, PythonStdState> for PyObjectGenerator {
        fn generate(&mut self, state: &mut PythonStdState) -> Result<BytesInput, Error> {
            let bytes = Python::with_gil(|py| -> PyResult<Vec<u8>> {
                self.inner
                    .call_method1(py, "generate", (PythonStdStateWrapper::wrap(state),))?
                    .extract(py)
            })
            .unwrap();
            Ok(BytesInput::new(bytes))
        }

        fn generate_dummy(&self, state: &mut PythonStdState) -> BytesInput {
            let bytes = Python::with_gil(|py| -> PyResult<Vec<u8>> {
                self.inner
                    .call_method1(py, "generate_dummy", (PythonStdStateWrapper::wrap(state),))?
                    .extract(py)
            })
            .unwrap();
            BytesInput::new(bytes)
        }
    }

    #[pyclass(unsendable, name = "RandBytesGenerator")]
    #[derive(Debug, Clone)]
    /// Python class for RandBytesGenerator
    pub struct PythonRandBytesGenerator {
        /// Rust wrapped RandBytesGenerator object
        pub inner: RandBytesGenerator<State = PythonStdState>,
    }

    #[pymethods]
    impl PythonRandBytesGenerator {
        #[new]
        fn new(max_size: usize) -> Self {
            Self {
                inner: RandBytesGenerator::new(max_size),
            }
        }

        fn generate(&mut self, state: &mut PythonStdStateWrapper) -> Vec<u8> {
            self.inner
                .generate(state.unwrap_mut())
                .expect("PythonRandBytesGenerator::generate failed")
                .bytes()
                .to_vec()
        }

        fn as_generator(slf: Py<Self>) -> PythonGenerator {
            PythonGenerator::new_rand_bytes(slf)
        }
    }

    #[pyclass(unsendable, name = "RandPrintablesGenerator")]
    #[derive(Debug, Clone)]
    /// Python class for RandPrintablesGenerator
    pub struct PythonRandPrintablesGenerator {
        /// Rust wrapped RandPrintablesGenerator object
        pub inner: RandPrintablesGenerator<PythonStdState>,
    }

    #[pymethods]
    impl PythonRandPrintablesGenerator {
        #[new]
        fn new(max_size: usize) -> Self {
            Self {
                inner: RandPrintablesGenerator::new(max_size),
            }
        }

        fn generate(&mut self, state: &mut PythonStdStateWrapper) -> Vec<u8> {
            self.inner
                .generate(state.unwrap_mut())
                .expect("PythonRandPrintablesGenerator::generate failed")
                .bytes()
                .to_vec()
        }

        fn as_generator(slf: Py<Self>) -> PythonGenerator {
            PythonGenerator::new_rand_printables(slf)
        }
    }

    #[derive(Debug, Clone)]
    enum PythonGeneratorWrapper {
        RandBytes(Py<PythonRandBytesGenerator>),
        RandPrintables(Py<PythonRandPrintablesGenerator>),
        Python(PyObjectGenerator),
    }

    /// Rand Trait binding
    #[pyclass(unsendable, name = "Generator")]
    #[derive(Debug, Clone)]
    pub struct PythonGenerator {
        wrapper: PythonGeneratorWrapper,
    }

    macro_rules! unwrap_me {
        ($wrapper:expr, $name:ident, $body:block) => {
            crate::unwrap_me_body!($wrapper, $name, $body, PythonGeneratorWrapper,
                { RandBytes, RandPrintables },
                {
                    Python(py_wrapper) => {
                        let $name = py_wrapper;
                        $body
                    }
                }
            )
        };
    }

    macro_rules! unwrap_me_mut {
        ($wrapper:expr, $name:ident, $body:block) => {
            crate::unwrap_me_mut_body!($wrapper, $name, $body, PythonGeneratorWrapper,
                { RandBytes, RandPrintables },
                {
                    Python(py_wrapper) => {
                        let $name = py_wrapper;
                        $body
                    }
                }
            )
        };
    }

    #[pymethods]
    impl PythonGenerator {
        #[staticmethod]
        fn new_rand_bytes(py_gen: Py<PythonRandBytesGenerator>) -> Self {
            Self {
                wrapper: PythonGeneratorWrapper::RandBytes(py_gen),
            }
        }

        #[staticmethod]
        fn new_rand_printables(py_gen: Py<PythonRandPrintablesGenerator>) -> Self {
            Self {
                wrapper: PythonGeneratorWrapper::RandPrintables(py_gen),
            }
        }

        #[staticmethod]
        #[must_use]
        pub fn new_py(obj: PyObject) -> Self {
            Self {
                wrapper: PythonGeneratorWrapper::Python(PyObjectGenerator::new(obj)),
            }
        }

        #[must_use]
        pub fn unwrap_py(&self) -> Option<PyObject> {
            match &self.wrapper {
                PythonGeneratorWrapper::Python(pyo) => Some(pyo.inner.clone()),
                _ => None,
            }
        }
    }

    impl Generator<BytesInput, PythonStdState> for PythonGenerator {
        fn generate(&mut self, state: &mut PythonStdState) -> Result<BytesInput, Error> {
            unwrap_me_mut!(self.wrapper, g, { g.generate(state) })
        }

        fn generate_dummy(&self, state: &mut PythonStdState) -> BytesInput {
            unwrap_me!(self.wrapper, g, { g.generate_dummy(state) })
        }
    }

    /// Register the classes to the python module
    pub fn register(_py: Python, m: &PyModule) -> PyResult<()> {
        m.add_class::<PythonRandBytesGenerator>()?;
        m.add_class::<PythonRandPrintablesGenerator>()?;
        m.add_class::<PythonGenerator>()?;
        Ok(())
    }
}
