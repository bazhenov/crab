use crate::{prelude::*, PageParser, PageTypeId, ParsedTables};
use pyo3::{
    prelude::*,
    types::{PyDict, PyList, PyTuple},
    PyErr,
};
use std::collections::HashMap;

pub struct PythonPageParser {
    module_name: String,
    page_type_id: PageTypeId,
    navigate_func: Option<PyObject>,
    parse_func: Option<PyObject>,
    validate_func: Option<PyObject>,
}

impl PythonPageParser {
    pub fn new(module_name: &str) -> Result<Self> {
        Python::with_gil(|py| {
            let module_name = module_name.to_string();
            let module = PyModule::import(py, module_name.as_str())?;
            let navigate_func = module.getattr("navigate").map(Into::into).ok();
            let parse_func = module.getattr("parse").map(Into::into).ok();
            let validate_func = module.getattr("validate").map(Into::into).ok();
            let page_type_id: PyObject = module.getattr("TYPE_ID").map(Into::into)?;
            let page_type_id = page_type_id.extract::<u8>(py)?;
            Ok(Self {
                module_name,
                navigate_func,
                parse_func,
                validate_func,
                page_type_id,
            })
        })
    }

    pub fn module_name(&self) -> &str {
        &self.module_name
    }

    pub fn support_navigation(&self) -> bool {
        self.navigate_func.is_some()
    }

    pub fn support_parsing(&self) -> bool {
        self.parse_func.is_some()
    }

    pub fn support_validation(&self) -> bool {
        self.validate_func.is_some()
    }
}

impl PageParser for PythonPageParser {
    fn navigate(&self, content: &str) -> Result<Option<Vec<(String, crate::PageTypeId)>>> {
        let Some(navigate) = &self.navigate_func else {
            return Ok(None)
        };
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
    }

    fn parse(&self, content: &str) -> Result<Option<ParsedTables>> {
        let Some(parse) = &self.parse_func else {
            return Ok(None)
        };
        let tables = Python::with_gil(|py| {
            let args = PyTuple::new(py, [content]);
            let return_value = parse.call1(py, args)?;
            let return_value = return_value.downcast::<PyDict>(py)?;

            let mut tables = HashMap::new();
            for (table_name, table) in return_value.into_iter() {
                let table_name = table_name.extract::<String>()?;
                let mut rows = vec![];
                for row in table.downcast::<PyList>()? {
                    rows.push(to_hashmap(row.downcast::<PyDict>()?)?);
                }
                tables.insert(table_name, rows);
            }
            Ok::<_, PyErr>(tables)
        })?;

        Ok(Some(tables))
    }

    fn validate(&self, content: &str) -> Result<bool> {
        let Some(validate) = &self.validate_func else {
            return Ok(true)
        };
        let valid = Python::with_gil(|py| {
            let args = PyTuple::new(py, [content]);
            let result = validate.call1(py, args)?;
            let valid = result.extract::<bool>(py)?;
            Ok::<_, PyErr>(valid)
        })?;
        Ok(valid)
    }

    fn page_type_id(&self) -> crate::PageTypeId {
        self.page_type_id
    }
}

fn to_hashmap(input: &PyDict) -> StdResult<HashMap<String, String>, PyErr> {
    let mut result = HashMap::new();
    for (column, value) in input.iter() {
        let column = column.extract::<String>()?;
        let value = value.extract::<String>()?;
        result.insert(column, value);
    }
    Ok(result)
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
