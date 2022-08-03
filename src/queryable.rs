use std::{collections::HashMap, sync::Arc};

use pyo3::prelude::*;
use zenoh::{
    queryable::{CallbackQueryable, Query},
    selector::ValueSelector,
};
use zenoh_core::SyncResolve;

use crate::{
    keyexpr::{_KeyExpr, _Selector},
    value::_Sample,
    ToPyErr,
};

#[pyclass(subclass)]
#[derive(Clone)]
pub struct _Query(pub(crate) Arc<Query>);
#[pymethods]
impl _Query {
    #[new]
    pub fn pynew(this: Self) -> Self {
        this
    }
    #[getter]
    pub fn key_expr(&self) -> _KeyExpr {
        _KeyExpr(self.0.key_expr().clone())
    }
    #[getter]
    pub fn value_selector(&self) -> &str {
        self.0.value_selector()
    }
    pub fn decode_value_selector(&self) -> PyResult<HashMap<String, String>> {
        let mut res = HashMap::new();
        for (k, v) in self.0.value_selector().decode() {
            let k = k.into_owned();
            match res.entry(k) {
                std::collections::hash_map::Entry::Occupied(e) => {
                    return Err(zenoh_core::zerror!(
                        "Detected duplicate key {} in value selector {}",
                        e.key(),
                        self.0.value_selector()
                    )
                    .to_pyerr())
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(v.into_owned());
                }
            }
        }
        Ok(res)
    }
    #[getter]
    pub fn selector(&self) -> _Selector {
        _Selector(self.0.selector().clone().into_owned())
    }
    pub fn reply(&self, sample: _Sample) -> PyResult<()> {
        self.0
            .reply(Ok(sample.into()))
            .res_sync()
            .map_err(|e| e.to_pyerr())
    }
}

#[pyclass(subclass)]
pub struct _Queryable(pub(crate) CallbackQueryable<'static>);
