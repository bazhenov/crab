use std::fs;

use pyo3::prelude::*;
use pyo3::types::{PyList, PyString, PyTuple};

fn main() -> PyResult<()> {
    let args = std::env::args().collect::<Vec<_>>();
    match (args.get(1), args.get(2), args.get(3)) {
        (Some(module_name), Some(function_name), Some(file_name)) => {
            pyo3::prepare_freethreaded_python();
            Python::with_gil(|py| {
                // Prependig current durectory to search path
                py.run(
                    r#"import sys
if '' not in sys.path:
    sys.path = [''] + sys.path"#,
                    None,
                    None,
                )
                .unwrap();
                let fun: Py<PyAny> = PyModule::import(py, module_name.as_str())?
                    .getattr(function_name.as_str())?
                    .into();

                let file = String::from_utf8_lossy(&fs::read(file_name)?).to_string();
                let args = PyTuple::new(py, &[&file]);
                // call object without any arguments
                let result = fun.call1(py, args)?;
                let list = result.downcast::<PyList>(py)?;
                for url in list {
                    let url = url.downcast::<PyString>()?.to_str()?;
                    println!("- {}", url);
                }
                Ok(())
            })
        }
        _ => Ok(()),
    }
}
