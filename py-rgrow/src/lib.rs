use pyo3::prelude::*;

#[pymodule]
#[pyo3(name = "rgrow")]
fn pyrgrow(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<rgrow::tileset::TileSet>()?;
    m.add_class::<rgrow::tileset::TileShape>()?;

    m.add_class::<rgrow::system::BoxedSystem>()?;
    m.add_class::<rgrow::state::BoxedState>()?;

    m.add_class::<rgrow::ffs::BoxedFFSResult>()?;
    m.add_class::<rgrow::ffs::FFSLevelRef>()?;

    m.add_class::<rgrow::ffs::FFSRunConfig>()?;
    m.add_class::<rgrow::system::EvolveBounds>()?;
    m.add_class::<rgrow::system::EvolveOutcome>()?;

    Ok(())
}
