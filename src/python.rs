use crate::{prelude::*, PageParser, PageTypeId, Pairs};
use pyo3::{
    prelude::*,
    types::{PyDict, PyList, PyTuple},
    PyErr,
};
use std::collections::HashMap;

pub struct PythonPageParser {
    type_id: PageTypeId,
    navigate_func: Option<PyObject>,
    parse_func: Option<PyObject>,
    validate_func: Option<PyObject>,
}

impl PythonPageParser {
    pub fn new(module_name: &str, type_id: PageTypeId) -> Result<Self> {
        Python::with_gil(|py| {
            let module = PyModule::import(py, module_name)?;
            let navigate_func = module.getattr("navigate").map(Into::into).ok();
            let parse_func = module.getattr("parse").map(Into::into).ok();
            let validate_func = module.getattr("validate").map(Into::into).ok();
            Ok(Self {
                navigate_func,
                parse_func,
                validate_func,
                type_id,
            })
        })
    }
}

impl PageParser for PythonPageParser {
    fn navigate(&self, content: &str) -> Result<Option<Vec<(String, crate::PageTypeId)>>> {
        if let Some(navigate) = &self.navigate_func {
            let list = Python::with_gil(|py| {
                let args = PyTuple::new(py, [content]);
                let result = navigate.call1(py, args)?;
                let mut urls = vec![];
                for tuple in result.downcast::<PyList>(py)? {
                    let url = tuple.get_item(0)?.extract::<String>()?;
                    let type_id = tuple.get_item(1)?.extract::<u8>()?;
                    urls.push((url, type_id));
                }
                Ok::<_, PyErr>(urls)
            })?;

            Ok(Some(list))
        } else {
            Ok(None)
        }
    }

    fn parse(&self, content: &str) -> Result<Option<Pairs>> {
        if let Some(parse) = &self.parse_func {
            let pairs = Python::with_gil(|py| {
                let args = PyTuple::new(py, [content]);
                let result = parse.call1(py, args)?;
                let dict = result.downcast::<PyDict>(py)?;

                let mut pairs = HashMap::new();
                for (key, value) in dict.into_iter() {
                    let key = key.extract::<String>()?;
                    let value = value.extract::<String>()?;
                    pairs.insert(key, value);
                }
                Ok::<_, PyErr>(pairs)
            })?;

            Ok(Some(pairs))
        } else {
            Ok(None)
        }
    }

    fn validate(&self, content: &str) -> Result<bool> {
        if let Some(validate) = &self.validate_func {
            let valid = Python::with_gil(|py| {
                let args = PyTuple::new(py, [content]);
                let result = validate.call1(py, args)?;
                let valid = result.extract::<bool>(py)?;
                Ok::<_, PyErr>(valid)
            })?;
            Ok(valid)
        } else {
            Ok(true)
        }
    }

    fn page_type_id(&self) -> crate::PageTypeId {
        self.type_id
    }
}

pub fn prepare() {
    pyo3::prepare_freethreaded_python();

    // Ensuring current working durectory is in Python search path
    {
        let py_code = r#"import sys
if '' not in sys.path:
    sys.path = [''] + sys.path"#;
        Python::with_gil(|py| {
            py.run(py_code, None, None).unwrap();
        })
    }
}
