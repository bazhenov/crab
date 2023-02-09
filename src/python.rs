use crate::{prelude::*, Page, PageParser, PageType};
use pyo3::{
    prelude::*,
    types::{PyList, PyString, PyTuple},
};
use reqwest::Url;

// TODO no need to do it public after refactoring
pub struct PythonPageParser {
    page_type: PageType,
    navigate_func: Py<PyAny>,
    parse_func: Py<PyAny>,
    validate_func: Py<PyAny>,
}

impl PythonPageParser {
    pub fn new(module_name: &str, page_type: PageType) -> Result<Self> {
        Python::with_gil(|py| {
            let module = PyModule::import(py, module_name)?;
            let navigate_func: Py<PyAny> = module.getattr("navigate")?.into();
            let parse_func: Py<PyAny> = module.getattr("parse")?.into();
            let validate_func: Py<PyAny> = module.getattr("validate")?.into();
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
        let list = Python::with_gil(|py| {
            let args = PyTuple::new(py, [content]);
            let result = self.navigate_func.call1(py, args)?;
            let list = result.downcast::<PyList>(py)?;

            let mut urls = vec![];
            for item in list {
                let url = item
                    .get_item(0)?
                    .downcast::<PyString>()?
                    .to_str()?
                    .to_string();
                let page_type = item.get_item(1)?.extract::<u8>()?;
                urls.push((url, page_type));
            }
            Ok::<_, pyo3::PyErr>(urls)
        })?;

        list.into_iter()
            .map(|i| create_absolute_url(i, page))
            .collect::<Result<Vec<_>>>()
            .map(Some)
    }

    fn kv(&self, content: &str) -> Result<Option<std::collections::HashMap<String, String>>> {
        todo!()
    }

    fn page_type(&self) -> crate::PageType {
        self.page_type
    }
}

// TODO no need to do it public after refactoring
pub fn prepare(module_path: &str) {
    pyo3::prepare_freethreaded_python();
    // TODO pass module path safely to the script
    let py_code = format!(
        r#"import sys
if '{0}' not in sys.path:
    sys.path = ['{0}'] + sys.path"#,
        module_path
    );
    Python::with_gil(|py| {
        // Prependig current directory to search path
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
