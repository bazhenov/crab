use crate::{prelude::*, Page, PageParser, PageType};
use pyo3::{
    prelude::*,
    types::{PyDict, PyList, PyTuple},
};
use reqwest::Url;
use std::collections::HashMap;

// TODO no need to do it public after refactoring
pub struct PythonPageParser {
    page_type: PageType,
    navigate_func: Option<PyObject>,
    parse_func: Option<PyObject>,
    validate_func: Option<PyObject>,
}

impl PythonPageParser {
    pub fn new(module_name: &str, page_type: PageType) -> Result<Self> {
        Python::with_gil(|py| {
            let module = PyModule::import(py, module_name)?;
            let navigate_func = module.getattr("navigate").map(Into::into).ok();
            let parse_func = module.getattr("parse").map(Into::into).ok();
            let validate_func = module.getattr("validate").map(Into::into).ok();
            Ok(Self {
                navigate_func,
                parse_func,
                validate_func,
                page_type,
            })
        })
    }
}

impl PageParser for PythonPageParser {
    fn next_pages(
        &self,
        page: &crate::Page,
        content: &str,
    ) -> Result<Option<Vec<(reqwest::Url, crate::PageType)>>> {
        if let Some(navigate) = &self.navigate_func {
            let list = Python::with_gil(|py| {
                let args = PyTuple::new(py, [content]);
                let result = navigate.call1(py, args)?;
                let mut urls = vec![];
                for url in result.downcast::<PyList>(py)? {
                    let page_type = url.get_item(1)?.extract::<u8>()?;
                    let url = url.get_item(0)?.extract::<String>()?;
                    urls.push((url, page_type));
                }
                Ok::<_, pyo3::PyErr>(urls)
            })?;

            list.into_iter()
                .map(|i| create_absolute_url(i, page))
                .collect::<Result<Vec<_>>>()
                .map(Some)
        } else {
            Ok(None)
        }
    }

    fn kv(&self, content: &str) -> Result<Option<HashMap<String, String>>> {
        if let Some(parse) = &self.parse_func {
            let kv = Python::with_gil(|py| {
                let args = PyTuple::new(py, [content]);
                let result = parse.call1(py, args)?;
                let dict = result.downcast::<PyDict>(py)?;

                let mut kv = HashMap::new();
                for (key, value) in dict {
                    let key = key.extract::<String>()?;
                    let value = value.extract::<String>()?;
                    kv.insert(key, value);
                }
                Ok::<_, pyo3::PyErr>(kv)
            })?;

            Ok(Some(kv))
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
                Ok::<_, pyo3::PyErr>(valid)
            })?;
            Ok(valid)
        } else {
            Ok(true)
        }
    }

    fn page_type(&self) -> crate::PageType {
        self.page_type
    }
}

// TODO no need to do it public after refactoring
pub fn prepare() {
    pyo3::prepare_freethreaded_python();
    // TODO pass module path safely to the script
    let py_code = r#"import sys
if '' not in sys.path:
    sys.path = [''] + sys.path"#;
    Python::with_gil(|py| {
        py.run(&py_code, None, None).unwrap();
    })
}

fn create_absolute_url(item: (String, PageType), page: &Page) -> Result<(Url, PageType)> {
    let (url, page_type) = item;
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok((Url::parse(&url)?, page_type))
    } else {
        Ok((page.url.join(&url)?, page_type))
    }
}
