use pyo3::prelude::*;

#[pymodule]
#[pyo3(name = "risio")]
fn risio(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ImageF64>()?;
    Ok(())
}

#[pyclass]
struct ImageF64 {}
