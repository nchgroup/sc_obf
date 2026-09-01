use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::ffi::CString;

pub fn python_is_available() -> bool {
    std::panic::catch_unwind(|| Python::attach(|_| ())).is_ok()
}

pub fn run_python_script(script: &str, shellcode: &[u8]) -> Result<String, String> {
    let code = CString::new(script).map_err(|_| "Script contains null bytes".to_string())?;
    Python::attach(|py| -> PyResult<String> {
        let module = PyModule::from_code(py, code.as_c_str(), c"script.py", c"user_script")?;
        let process_fn = module.getattr("process").map_err(|_| {
            pyo3::exceptions::PyAttributeError::new_err(
                "Script must define a 'process(shellcode: bytes) -> str' function",
            )
        })?;
        let sc_bytes = PyBytes::new(py, shellcode);
        let result = process_fn.call1((sc_bytes,))?;
        result.extract::<String>()
    })
    .map_err(|e| format!("{e}"))
}
