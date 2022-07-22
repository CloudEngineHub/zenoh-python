use crate::ConfigNotifier;

//
// Copyright (c) 2017, 2022 ZettaScale Technology Inc.
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh team, <zenoh@zettascale.tech>
//
use super::encoding::Encoding;
use super::sample_kind::SampleKind;
use super::types::{
    zkey_expr_of_pyany, zvalue_of_pyany, CongestionControl, KeyExpr, Period, Priority, Query,
    QueryConsolidation, QueryTarget, Queryable, Reliability, Reply, Sample, SubMode, Subscriber,
};
use super::{to_pyerr, ZError};
use log::warn;
use pyo3::exceptions;
use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyDict, PyList, PyTuple};
use std::collections::HashMap;
use std::sync::Arc;
use zenoh::prelude::sync::SyncResolve;
use zenoh::prelude::{KeyExpr as ZKeyExpr, *};
use zenoh::subscriber::CallbackSubscriber;

/// A zenoh session.
#[pyclass]
pub struct Session {
    s: Option<Arc<zenoh::Session>>,
}

#[pymethods]
impl Session {
    /// Returns the identifier for this session.
    ///
    /// :type: **str**
    #[getter]
    fn id(&self) -> PyResult<String> {
        let s = self.try_ref()?;
        Ok(s.id())
    }

    /// Close the zenoh Session.
    pub fn close(&mut self) -> PyResult<()> {
        let s = self.try_take()?;
        match Arc::try_unwrap(s) {
            Ok(s) => s.close().res().map_err(to_pyerr),
            Err(_) => Err(PyErr::new::<exceptions::PyValueError, _>(
                "Failed to close Session: not owner of the last reference",
            )),
        }
    }

    /// Get informations about the zenoh Session.
    ///
    /// :rtype: **dict[str, str]**
    ///
    /// :Example:
    ///
    /// >>> import zenoh
    /// >>> s = zenoh.open()
    /// >>> info = s.info()
    /// >>> for key in info:
    /// >>>    print("{} : {}".format(key, info[key]))
    pub fn info(&self, py: Python) -> PyResult<PyObject> {
        use zenoh_cfg_properties::KeyTranscoder;
        let s = self.try_ref()?;
        let props = s.info().res();
        let pydict: HashMap<String, String> = props
            .0
            .into_iter()
            .filter_map(|(k, v)| zenoh::info::InfoTranscoder::decode(k).map(|k| (k, v)))
            .collect();
        Ok(pydict.into_py_dict(py).to_object(py))
    }

    /// Get informations about the zenoh Session.
    ///
    /// :rtype:  **dict[str, str]**
    ///
    /// :Example:
    ///
    /// >>> import zenoh
    /// >>> s = zenoh.open()
    /// >>> info = s.info()
    /// >>> for key in info:
    /// >>>    print("{} : {}".format(key, info[key]))
    ///
    ///
    /// Get the current configuration of the zenoh Session.
    ///
    /// The returned ConfigNotifier can be used to read the current
    /// zenoh configuration through the json function or
    /// modify the zenoh configuration through the insert_json5 funtion.
    ///
    /// :rtype: dict {str: str}
    ///
    /// :Example:
    ///
    /// >>> import zenoh
    /// >>> s = zenoh.open()
    /// >>> config = s.config()
    /// >>> config.insert_json5("connect/endpoints", "[\"tcp/10.10.10.10:7448\"]")
    ///
    pub fn config(&self) -> PyResult<ConfigNotifier> {
        Ok(ConfigNotifier {
            inner: self.try_ref()?.config().clone(),
        })
    }

    /// Put data.
    ///
    /// :param key_expr: The key expression matching resources to write
    /// :type key_expr: a :class:`KeyExpr` or any type convertible to a :class:`KeyExpr`
    ///                 (see its constructor's accepted parameters)
    /// :param value: The value to write
    /// :type value: any type convertible to a :class:`Value`
    /// :param \**kwargs:
    ///    See below
    ///
    /// :Keyword Arguments:
    ///    * **encoding** (:class:`Encoding`) --
    ///      Set the encoding of the written data
    ///    * **kind** ( **int** ) --
    ///      Set the kind of the written data
    ///    * **congestion_control** (:class:`CongestionControl`) --
    ///      Set the congestion control to apply when routing the data
    ///    * **priority** (:class:`Priority`) --
    ///      Set the priority of the written data
    ///    * **local_routing** ( **bool** ) --
    ///      Enable or disable local routing
    ///
    /// :raise: :class:`ZError`
    ///
    /// :Examples:
    ///
    /// >>> import zenoh
    /// >>> s = zenoh.open()
    /// >>> s.put('/key/expression', 'value')
    #[pyo3(text_signature = "(self, key_expr, value, **kwargs)")]
    #[args(kwargs = "**")]
    pub fn put(&self, key_expr: &PyAny, value: &PyAny, kwargs: Option<&PyDict>) -> PyResult<()> {
        let s = self.try_ref()?;
        let k = zkey_expr_of_pyany(key_expr)?;
        let v = zvalue_of_pyany(value)?;
        let mut builder = s.put(k, v);
        if let Some(kwargs) = kwargs {
            if let Some(arg) = kwargs.get_item("encoding") {
                builder = builder.encoding(arg.extract::<Encoding>()?.e);
            }
            if let Some(arg) = kwargs.get_item("kind") {
                builder = builder.kind(arg.extract::<SampleKind>()?.kind);
            }
            if let Some(arg) = kwargs.get_item("congestion_control") {
                builder = builder.congestion_control(arg.extract::<CongestionControl>()?.cc);
            }
            if let Some(arg) = kwargs.get_item("priority") {
                builder = builder.priority(arg.extract::<Priority>()?.p);
            }
            if let Some(arg) = kwargs.get_item("local_routing") {
                builder = builder.local_routing(arg.extract::<bool>()?);
            }
        }
        builder.res().map_err(to_pyerr)
    }

    /// Delete data.
    ///
    /// :param key_expr: The key expression matching resources to delete
    /// :type key_expr: a :class:`KeyExpr` or any type convertible to a :class:`KeyExpr`
    ///                 (see its constructor's accepted parameters)
    /// :param \**kwargs:
    ///    See below
    ///
    /// :Keyword Arguments:
    ///    * **congestion_control** (:class:`CongestionControl`) --
    ///      Set the congestion control to apply when routing the data
    ///    * **priority** (:class:`Priority`) --
    ///      Set the priority of the written data
    ///    * **local_routing** ( **bool** ) --
    ///      Enable or disable local routing
    ///
    /// :raise: :class:`ZError`
    ///
    /// :Examples:
    ///
    /// >>> import zenoh
    /// >>> s = zenoh.open()
    /// >>> s.delete('/key/expression')
    #[pyo3(text_signature = "(self, key_expr, **kwargs)")]
    #[args(kwargs = "**")]
    pub fn delete(&self, key_expr: &PyAny, kwargs: Option<&PyDict>) -> PyResult<()> {
        let s = self.try_ref()?;
        let k = zkey_expr_of_pyany(key_expr)?;
        let mut builder = s.delete(k);
        if let Some(kwargs) = kwargs {
            if let Some(arg) = kwargs.get_item("congestion_control") {
                builder = builder.congestion_control(arg.extract::<CongestionControl>()?.cc);
            }
            if let Some(arg) = kwargs.get_item("priority") {
                builder = builder.priority(arg.extract::<Priority>()?.p);
            }
            if let Some(arg) = kwargs.get_item("local_routing") {
                builder = builder.local_routing(arg.extract::<bool>()?);
            }
        }
        builder.res().map_err(to_pyerr)
    }

    /// Associate a numerical Id with the given key expression.
    ///
    /// This numerical Id will be used on the network to save bandwidth and
    /// ease the retrieval of the concerned resource in the routing tables.
    ///
    /// :param key_expr: The key expression to map to a numerical Id
    /// :type key_expr: a :class:`KeyExpr` or any type convertible to a :class:`KeyExpr`
    ///                 (see its constructor's accepted parameters)
    /// :rtype: **int**
    /// :raise: :class:`ZError`
    ///
    /// :Examples:
    ///
    /// >>> import zenoh
    /// >>> s = zenoh.open()
    /// >>> rid = s.declare_expr('/key/expression')
    #[pyo3(text_signature = "(self, key_expr)")]
    pub fn declare_expr(&self, key_expr: &PyAny) -> PyResult<ExprId> {
        let s = self.try_ref()?;
        let k = zkey_expr_of_pyany(key_expr)?;
        s.declare_expr(&k).res().map_err(to_pyerr)
    }

    /// Undeclare the *numerical Id/key expression* association previously declared
    /// with :meth:`declare_expr`.
    ///
    /// :param rid: The numerical Id to unmap
    /// :type rid: :class:`ExprId`
    /// :raise: :class:`ZError`
    ///
    /// :Examples:
    ///
    /// >>> import zenoh
    /// >>> s = zenoh.open()
    /// >>> rid = s.declare_expr('/key/expression')
    /// >>> s.undeclare_expr(rid)
    #[pyo3(text_signature = "(self, rid)")]
    pub fn undeclare_expr(&self, rid: ExprId) -> PyResult<()> {
        let s = self.try_ref()?;
        s.undeclare_expr(rid).res().map_err(to_pyerr)
    }

    /// Declare a publication for the given key expression.
    ///
    /// Written expressions that match the given key expression will only be sent on the network
    /// if matching subscribers exist in the system.
    ///
    /// :param key_expr: The key expression to publish
    /// :type key_expr: a :class:`KeyExpr` or any type convertible to a :class:`KeyExpr`
    ///                 (see its constructor's accepted parameters)
    /// :raise: :class:`ZError`
    ///
    /// :Examples:
    ///
    /// >>> import zenoh
    /// >>> s = zenoh.open()
    /// >>> rid = s.declare_publication('/key/expression')
    /// >>> s.put('/key/expression', bytes('value', encoding='utf8'))
    #[pyo3(text_signature = "(self, key_expr)")]
    fn declare_publication(&self, key_expr: &PyAny) -> PyResult<()> {
        let s = self.try_ref()?;
        let k = zkey_expr_of_pyany(key_expr)?;
        s.declare_publication(&k).res().map_err(to_pyerr)?;
        Ok(())
    }

    /// Undeclare the publication previously declared with :meth:`declare_publication`.
    ///
    /// :param key_expr: The same key expression that was used to declare the publication
    /// :type key_expr: a :class:`KeyExpr` or any type convertible to a :class:`KeyExpr`
    ///                 (see its constructor's accepted parameters)
    /// :raise: :class:`ZError`
    #[pyo3(text_signature = "(self, key_expr)")]
    fn undeclare_publication(&self, key_expr: &PyAny) -> PyResult<()> {
        let s = self.try_ref()?;
        let k = zkey_expr_of_pyany(key_expr)?;
        s.undeclare_publication(&k).res().map_err(to_pyerr)?;
        Ok(())
    }

    /// Create a Subscriber for the given key expression.
    ///
    /// :param key_expr: The key expression to subscribe
    /// :type key_expr: a :class:`KeyExpr` or any type convertible to a :class:`KeyExpr`
    ///                 (see its constructor's accepted parameters)
    /// :param callback: the subscription callback
    /// :type callback: function(:class:`Sample`)
    /// :param \**kwargs:
    ///    See below
    ///
    /// :Keyword Arguments:
    ///    * **reliability** (:class:`Reliability`) --
    ///      Set the subscription reliability (BestEffort by default)
    ///    * **mode** (:class:`SubMode`) --
    ///      Set the subscription mode (Push by default)
    ///    * **period** (:class:`Period`) --
    ///      Set the subscription period
    ///    * **local** ( **bool** ) --
    ///      If true make the subscription local only (false by default)
    ///
    /// :rtype: :class:`Subscriber`
    /// :raise: :class:`ZError`
    ///
    /// :Examples:
    ///
    /// >>> import zenoh, time
    /// >>> from zenoh import Reliability, SubMode
    /// >>>
    /// >>> s = zenoh.open()
    /// >>> sub = s.subscribe('/key/expression',
    /// ...     lambda sample: print("Received : {}".format(sample)),
    /// ...     reliability=Reliability.Reliable,
    /// ...     mode=SubMode.Push)
    /// >>> time.sleep(60)
    #[pyo3(text_signature = "(self, key_expr, callback, **kwargs)")]
    #[args(kwargs = "**")]
    fn subscribe(
        &self,
        key_expr: &PyAny,
        callback: &PyAny,
        kwargs: Option<&PyDict>,
    ) -> PyResult<Subscriber> {
        let s = self.try_ref()?;
        let k = zkey_expr_of_pyany(key_expr)?;
        let mut builder = s.subscribe(&k);
        if let Some(kwargs) = kwargs {
            if let Some(arg) = kwargs.get_item("reliability") {
                builder = builder.reliability(arg.extract::<Reliability>()?.r);
            }
            if let Some(arg) = kwargs.get_item("mode") {
                builder = builder.mode(arg.extract::<SubMode>()?.m);
            }
            if let Some(arg) = kwargs.get_item("period") {
                builder = builder.period(Some(arg.extract::<Period>()?.p));
            }
            if let Some(arg) = kwargs.get_item("local") {
                if arg.extract::<bool>()? {
                    builder = builder.local();
                }
            }
        }

        // Note: callback cannot be passed as such in task below because it's not Send
        let cb_obj: Py<PyAny> = callback.into();
        let z_sub: CallbackSubscriber<'static> = builder
            .callback(move |s| {
                // Acquire Python GIL to call the callback
                let gil = Python::acquire_gil();
                let py = gil.python();
                let cb_args = PyTuple::new(py, &[Sample { s }]);
                if let Err(e) = cb_obj.as_ref(py).call1(cb_args) {
                    warn!("Error calling subscriber callback:");
                    e.print(py);
                }
            })
            .res()
            .map_err(to_pyerr)?;
        Ok(Subscriber { inner: Some(z_sub) })
    }

    /// Create a Queryable for the given key expression.
    ///
    /// :param key_expr: The key expression the Queryable will reply to
    /// :type key_expr: a :class:`KeyExpr` or any type convertible to a :class:`KeyExpr`
    ///                 (see its constructor's accepted parameters)
    /// :param callback: the queryable callback
    /// :type callback: function(:class:`Query`)
    /// :param \**kwargs:
    ///    See below
    ///
    /// :Keyword Arguments:
    ///    * **complete** ( **bool** ) --
    ///      Set the queryable completeness (true by default)
    ///
    /// :rtype: :class:`Queryable`
    /// :raise: :class:`ZError`
    ///
    /// :Examples:
    ///
    /// >>> import zenoh, time
    /// >>> from zenoh import Sample, queryable
    /// >>> def callback(query):
    /// ...     print("Received : {}".format(query))
    /// ...     query.reply(Sample('/key/expression', bytes('value', encoding='utf8')))
    /// >>>
    /// >>> s = zenoh.open()
    /// >>> q = s.queryable('/key/expression', callback)
    /// >>> time.sleep(60)
    #[pyo3(text_signature = "(self, key_expr, callback, **kwargs)")]
    #[args(kwargs = "**")]
    fn queryable(
        &self,
        key_expr: &PyAny,
        callback: &PyAny,
        kwargs: Option<&PyDict>,
    ) -> PyResult<Queryable> {
        let s = self.try_ref()?;
        let k = zkey_expr_of_pyany(key_expr)?;
        let mut builder = s.queryable(k);
        if let Some(kwargs) = kwargs {
            if let Some(arg) = kwargs.get_item("complete") {
                builder = builder.complete(arg.extract::<bool>()?);
            }
        }

        // Note: callback cannot be passed as such in task below because it's not Send
        let cb_obj: Py<PyAny> = callback.into();
        let z_quer = builder
            .callback(move |q| {
                // Acquire Python GIL to call the callback
                let gil = Python::acquire_gil();
                let py = gil.python();
                let cb_args = PyTuple::new(
                    py,
                    &[Query {
                        q: async_std::sync::Arc::new(q),
                    }],
                );
                if let Err(e) = cb_obj.as_ref(py).call1(cb_args) {
                    warn!("Error calling queryable callback:");
                    e.print(py);
                }
            })
            .res()
            .map_err(to_pyerr)?;
        Ok(Queryable {
            inner: Some(z_quer),
        })
    }

    /// Query data from the matching queryables in the system.
    ///
    /// Replies are collected in a list.
    ///
    /// The *selector* parameter also accepts the following types that can be converted to a :class:`Selector`:
    ///
    /// * **KeyExpr** for a key expression with no value selector
    /// * **int** for a key expression id with no value selector
    /// * **str** for a litteral selector
    ///
    /// :param selector: The selection of resources to query
    /// :type selector: :class:`Selector`
    /// :param \**kwargs:
    ///    See below
    ///
    /// :Keyword Arguments:
    ///    * **target** (:class:`QueryTarget`) --
    ///      Set the kind of queryables that should be target of this query
    ///    * **consolidation** (:class:`QueryConsolidation`) --
    ///      Set the consolidation mode of the query
    ///    * **local_routing** ( **bool** ) --
    ///      Enable or disable local routing
    ///
    /// :rtype: [:class:`Reply`]
    /// :raise: :class:`ZError`
    ///
    /// :Examples:
    ///
    /// >>> import zenoh, time
    /// >>>
    /// >>> s = zenoh.open()
    /// >>> replies = s.get('/key/selector?value_selector')
    /// >>> for reply in replies:
    /// ...    print("Received : {}".format(reply.sample))
    #[pyo3(text_signature = "(self, selector, **kwargs)")]
    #[args(kwargs = "**")]
    fn get(&self, selector: &PyAny, kwargs: Option<&PyDict>) -> PyResult<Py<PyList>> {
        let s = self.try_ref()?;
        let selector: Selector = match selector.get_type().name()? {
            "KeyExpr" => {
                let key_expr: PyRef<KeyExpr> = selector.extract()?;
                key_expr.inner.clone().into()
            }
            "int" => {
                let id: u64 = selector.extract()?;
                ZKeyExpr::from(id).into()
            }
            "str" => {
                let name: &str = selector.extract()?;
                Selector::from(name)
            }
            x => {
                return Err(PyErr::new::<exceptions::PyValueError, _>(format!(
                    "Cannot convert type '{}' to a zenoh Selector",
                    x
                )))
            }
        };
        let mut builder = s.get(selector);
        if let Some(kwargs) = kwargs {
            if let Some(arg) = kwargs.get_item("target") {
                builder = builder.target(arg.extract::<QueryTarget>()?.t);
            }
            if let Some(arg) = kwargs.get_item("consolidation") {
                builder = builder.consolidation(arg.extract::<QueryConsolidation>()?.c);
            }
            if let Some(arg) = kwargs.get_item("local_routing") {
                builder = builder.local_routing(arg.extract::<bool>()?);
            }
        }
        let receiver = builder.res().map_err(to_pyerr)?;
        let gil = Python::acquire_gil();
        let py = gil.python();
        let result = PyList::empty(py);
        while let Ok(reply) = receiver.recv() {
            result.append(Reply { r: reply })?;
        }
        Ok(result.into())
    }

    /// Convert a :class:`KeyExpr` into the corresponding stringified key expression
    /// (i.e. the scope is converted its corresponding key expression and the suffix is concatenated).
    ///
    /// :param key_expr: The selection of resources to query
    /// :type key_expr: a :class:`KeyExpr` or any type convertible to a :class:`KeyExpr`
    ///                 (see its constructor's accepted parameters)
    ///
    /// :rtype: **str**
    /// :raise: :class:`ZError`
    #[pyo3(text_signature = "(self, key_expr)")]
    fn key_expr_to_expr(&self, key_expr: &KeyExpr) -> PyResult<String> {
        self.try_ref()?
            .key_expr_to_expr(&key_expr.inner)
            .map_err(to_pyerr)
    }
}

impl Session {
    pub(crate) fn new(s: zenoh::Session) -> Self {
        Session {
            s: Some(s.into_arc()),
        }
    }

    #[inline]
    fn try_ref(&self) -> PyResult<&Arc<zenoh::Session>> {
        self.s
            .as_ref()
            .ok_or_else(|| PyErr::new::<ZError, _>("zenoh session was closed"))
    }

    #[inline]
    fn try_take(&mut self) -> PyResult<Arc<zenoh::Session>> {
        self.s
            .take()
            .ok_or_else(|| PyErr::new::<ZError, _>("zenoh session was closed"))
    }
}
