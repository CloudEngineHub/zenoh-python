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
use crate::encoding::Encoding;
use crate::sample_kind::SampleKind;
use crate::{to_pyerr, ZError};
use log::warn;
use pyo3::exceptions;
use pyo3::number::PyNumberOrProtocol;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyTuple};
use pyo3::PyObjectProtocol;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::ops::BitOr;
use zenoh::config::whatami::WhatAmIMatcher;
use zenoh::config::WhatAmI as ZWhatAmI;
use zenoh::prelude::sync::SyncResolve;
use zenoh::prelude::{
    Encoding as ZEncoding, KeyExpr as ZKeyExpr, KnownEncoding as ZKnownEncoding, Sample as ZSample,
    Selector as ZSelector, Value as ZValue, ZInt,
};
use zenoh::queryable::CallbackQueryable;
use zenoh::subscriber::CallbackSubscriber;
use zenoh_buffers::traits::SplitBuffer;

// zenoh.config (simulate the package as a class, and consts as class attributes)
//
/// The following constants define the several configuration keys accepted for a zenoh
/// session configuration and the associated accepted values.
#[allow(non_camel_case_types)]
#[pyclass]
pub(crate) struct config {}

#[allow(non_snake_case)]
#[pymethods]
impl config {
    /// The library mode.
    ///
    /// - Accepted values : `"peer"`, `"client"`.
    /// - Default value : `"peer"`.
    #[classattr]
    pub fn MODE_KEY() -> &'static str {
        "mode"
    }

    /// The locator of a peer to connect to.
    ///
    /// - Accepted values : `<locator>` (ex: `"tcp/10.10.10.10:7447"`).
    /// - Default value : None.
    /// - Multiple values accepted.
    #[classattr]
    pub fn CONNECT_KEY() -> &'static str {
        "connect/endpoints"
    }

    /// A locator to listen on.
    ///
    /// - Accepted values : `<locator>` (ex: `"tcp/10.10.10.10:7447"`).
    /// - Default value : None.
    /// - Multiple values accepted.
    #[classattr]
    pub fn LISTEN_KEY() -> &'static str {
        "listen/endpoints"
    }

    /// The user name to use for authentication.
    ///
    /// - Accepted values : `<string>`.
    /// - Default value : None.
    #[classattr]
    pub fn USER_KEY() -> &'static str {
        "transport/auth/usrpwd/user"
    }

    /// The password to use for authentication.
    ///
    /// - Accepted values : `<string>`.
    /// - Default value : None.
    #[classattr]
    fn PASSWORD_KEY() -> &'static str {
        "transport/auth/usrpwd/password"
    }

    /// Activates/Desactivates multicast scouting.
    ///
    /// - Accepted values : `"true"`, `"false"`.
    /// - Default value : `"true"`.
    #[classattr]
    pub fn MULTICAST_SCOUTING_KEY() -> &'static str {
        "scouting/multicast/enabled"
    }

    /// The network interface to use for multicast scouting.
    ///
    /// - Accepted values : `"auto"`, `<ip address>`, `<interface name>`.
    /// - Default value : `"auto"`.
    #[classattr]
    pub fn MULTICAST_INTERFACE_KEY() -> &'static str {
        "scouting/multicast/interface"
    }

    /// The multicast address and ports to use for multicast scouting.
    ///
    /// - Accepted values : `<ip address>:<port>`.
    /// - Default value : `"224.0.0.224:7447"`.
    #[classattr]
    pub fn MULTICAST_IPV4_ADDRESS_KEY() -> &'static str {
        "scouting/multicast/address"
    }

    /// In client mode, the period dedicated to scouting a router before failing.
    ///
    /// - Accepted values : `<float in seconds>`.
    /// - Default value : `"3.0"`.
    #[classattr]
    pub fn SCOUTING_TIMEOUT_KEY() -> &'static str {
        "scouting/timeout"
    }

    /// In peer mode, the period dedicated to scouting first remote peers before doing anything else.
    ///
    /// - Accepted values : `<float in seconds>`.
    /// - Default value : `"0.2"`.
    #[classattr]
    pub fn SCOUTING_DELAY_KEY() -> &'static str {
        "scouting/delay"
    }

    /// Indicates if data messages should be timestamped.
    ///
    /// - Accepted values : `"true"`, `"false"`.
    /// - Default value : `"false"`.
    #[classattr]
    pub fn ADD_TIMESTAMP_KEY() -> &'static str {
        "add_timestamp"
    }

    /// Indicates if local writes/queries should reach local subscribers/queryables.
    #[classattr]
    pub fn LOCAL_ROUTING_KEY() -> &'static str {
        "local_routing"
    }
}

// zenoh.info (simulate the package as a class, and consts as class attributes)
/// Constants and helpers to interpret the properties returned by :func:`zenoh.Session.info`.
#[allow(non_camel_case_types)]
#[pyclass]
pub(crate) struct info {}

#[allow(non_snake_case)]
#[pymethods]
impl info {
    #[classattr]
    fn ZN_INFO_PID_KEY() -> ZInt {
        zenoh::info::ZN_INFO_PID_KEY
    }

    #[classattr]
    fn ZN_INFO_PEER_PID_KEY() -> ZInt {
        zenoh::info::ZN_INFO_PEER_PID_KEY
    }

    #[classattr]
    fn ZN_INFO_ROUTER_PID_KEY() -> ZInt {
        zenoh::info::ZN_INFO_ROUTER_PID_KEY
    }
}

// zenoh.whatami (simulate the package as a class, and consts as class attributes)
/// Constants defining the different zenoh process to look for with :func:`zenoh.scout`.
#[allow(non_camel_case_types)]
#[pyclass]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WhatAmI {
    inner: WhatAmIMatcher,
}
impl From<WhatAmI> for WhatAmIMatcher {
    fn from(w: WhatAmI) -> Self {
        w.inner
    }
}
impl BitOr for WhatAmI {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        WhatAmI {
            inner: self.inner | rhs.inner,
        }
    }
}
#[pyproto]
impl pyo3::PyNumberProtocol for WhatAmI
where
    <Self as PyNumberOrProtocol<'p>>::Left: BitOr<
        <Self as PyNumberOrProtocol<'p>>::Right,
        Output = <Self as PyNumberOrProtocol<'p>>::Result,
    >,
{
    fn __or__(lhs: Self, rhs: Self) -> Self
    where
        Self: PyNumberOrProtocol<'p>,
    {
        lhs | rhs
    }
}

#[allow(non_snake_case)]
#[pymethods]
impl WhatAmI {
    #[classattr]
    fn Router() -> Self {
        WhatAmI {
            inner: ZWhatAmI::Router.into(),
        }
    }

    #[classattr]
    fn Peer() -> Self {
        WhatAmI {
            inner: ZWhatAmI::Peer.into(),
        }
    }

    #[classattr]
    fn Client() -> Self {
        WhatAmI {
            inner: ZWhatAmI::Client.into(),
        }
    }
}

impl std::fmt::Display for WhatAmI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.to_str())
    }
}

/// A Hello message received as a response to a :meth:`scout`
#[pyclass]
#[derive(Clone)]
pub(crate) struct Hello {
    pub(crate) h: zenoh::scouting::Hello,
}

#[pymethods]
impl Hello {
    /// The ZenohId of the Hello message sender
    ///
    /// :type: :class:`ZenohId` or `None`
    #[getter]
    fn pid(&self) -> Option<ZenohId> {
        self.h.pid.as_ref().map(|p| ZenohId { p: *p })
    }

    /// The mode of the Hello message sender (bitmask of constants from :class:`whatami`)
    ///
    /// :type: :class:`whatami` or `None`
    #[getter]
    fn whatami(&self) -> Option<WhatAmI> {
        self.h.whatami.map(|w| WhatAmI { inner: w.into() })
    }

    /// The locators list of the Hello message sender
    ///
    /// :type: list of str or `None`
    #[getter]
    fn locators(&self) -> Option<Vec<String>> {
        self.h
            .locators
            .as_ref()
            .map(|v| v.iter().map(|l| l.to_string()).collect())
    }
}

#[pyproto]
impl PyObjectProtocol for Hello {
    fn __str__(&self) -> String {
        self.h.to_string()
    }
}

/// A zenoh **resource** is represented by a pair composed by a **key** and a
/// **value**, such as, ``(/car/telemetry/speed, 320)``.  A **resource key**
/// is an arbitrary array of characters, with the exclusion of the symbols
/// ``*``, ``**``, ``?``, ``[``, ``]``, and ``#``,
/// which have special meaning in the context of zenoh.
///
/// A key including any number of the wildcard symbols, ``*`` and ``**``,
/// such as, ``/car/telemetry/*``, is called a **key expression** as it
/// denotes a set of keys. The wildcard character ``*`` expands to an
/// arbitrary string not including zenoh's reserved characters and the ``/``
/// character, while the ``**`` expands to  strings that may also include the
/// ``/`` character.
///
/// Finally, it is worth mentioning that for time and space efficiency matters,
/// zenoh will automatically map key expressions to small integers. The mapping is automatic,
/// but it can be triggered excplicily by with :meth:`Session.declare_expr`.
///
/// The KeyExpr constructor accepts any of the following types as parameters:
///
/// * **int** for a mapped key expression (see :meth:`Session.declare_expr`)
/// * **str** for a literal key expression
/// * **(int, str)** for a mapped key expression with suffix
#[allow(non_camel_case_types)]
#[pyclass]
pub struct KeyExpr {
    pub(crate) inner: ZKeyExpr<'static>,
}

#[allow(non_snake_case)]
#[pymethods]
impl KeyExpr {
    #[new]
    fn new(key_expr: &PyAny) -> PyResult<Self> {
        let inner = zkey_expr_of_pyany(key_expr)?;
        Ok(KeyExpr {
            inner: inner.to_owned(),
        })
    }

    /// The numeric scope (0 marks global scope)
    #[getter]
    fn scope(&self) -> ZInt {
        self.inner.scope
    }

    /// The string suffix (or the complete key expression if scope==0)
    #[getter]
    fn suffix(&self) -> String {
        self.inner.suffix.to_string()
    }

    /// Return true if both resource names intersect.
    ///
    /// :param s1: the 1st resource name
    /// :type s1: str
    /// :param s2: the 2nd resource name
    /// :type s2: str
    #[staticmethod]
    #[pyo3(text_signature = "(s1, s2)")]
    fn intersect(s1: &PyAny, s2: &PyAny) -> bool {
        let s1 = zkey_expr_of_pyany(s1).unwrap();
        let s2 = zkey_expr_of_pyany(s2).unwrap();
        match (s1.as_id_and_suffix(), s2.as_id_and_suffix()) {
            ((s1, _), (s2, _)) if s1 != s2 => false,
            ((_, s1), (_, s2)) => zenoh::utils::key_expr::intersect(s1, s2),
        }
    }
}

#[pyproto]
impl PyObjectProtocol for KeyExpr {
    fn __str__(&self) -> PyResult<String> {
        Ok(self.inner.to_string())
    }
}

impl From<KeyExpr> for ZKeyExpr<'static> {
    fn from(r: KeyExpr) -> ZKeyExpr<'static> {
        r.inner
    }
}

impl From<ZKeyExpr<'static>> for KeyExpr {
    fn from(inner: ZKeyExpr<'static>) -> KeyExpr {
        KeyExpr { inner }
    }
}

pub(crate) fn zkey_expr_of_pyany(obj: &PyAny) -> PyResult<ZKeyExpr> {
    match obj.get_type().name()? {
        "KeyExpr" => {
            let rk: PyRef<KeyExpr> = obj.extract()?;
            Ok(rk.inner.clone())
        }
        "int" => {
            let id: u64 = obj.extract()?;
            Ok(id.into())
        }
        "str" => {
            let name: String = obj.extract()?;
            Ok(name.into())
        }
        "tuple" => {
            let tuple: &PyTuple = obj.downcast()?;
            if tuple.len() == 2
                && tuple.get_item(0)?.get_type().name()? == "int"
                && tuple.get_item(1)?.get_type().name()? == "str"
            {
                let id: u64 = tuple.get_item(0)?.extract()?;
                let suffix: String = tuple.get_item(1)?.extract()?;
                Ok(ZKeyExpr::from(id).with_suffix(&suffix).to_owned())
            } else {
                Err(PyErr::new::<exceptions::PyValueError, _>(format!(
                    "Cannot convert type '{:?}' to a zenoh-net KeyExpr",
                    tuple
                )))
            }
        }
        x => Err(PyErr::new::<exceptions::PyValueError, _>(format!(
            "Cannot convert type '{}' to a zenoh-net KeyExpr",
            x
        ))),
    }
}

/// An expression identifying a selection of resources.
///
/// A selector is the conjunction of an key expression identifying a set
/// of resource keys and a value selector filtering out the resource values.
///
/// Structure of a selector:::
///
///    /s1/s2/..../sn?x>1&y<2&...&z=4(p1=v1;p2=v2;...;pn=vn)[a;b;x;y;...;z]
///    |key_selector||---------------- value_selector --------------------|
///                   |--- filter --| |---- properties ---|  |--fragment-|
///
/// where:
///  * **key_selector**: an expression identifying a set of Resources.
///  * **filter**: a list of `value_selectors` separated by `'&'` allowing to perform filtering on the values
///    associated with the matching keys. Each `value_selector` has the form "`field`-`operator`-`value`" value where:
///
///      * *field* is the name of a field in the value (is applicable and is existing. otherwise the `value_selector` is false)
///      * *operator* is one of a comparison operators: `<` , `>` , `<=` , `>=` , `=` , `!=`
///      * *value* is the the value to compare the field’s value with
///
///  * **fragment**: a list of fields names allowing to return a sub-part of each value.
///    This feature only applies to structured values using a “self-describing” encoding, such as JSON or XML.
///    It allows to select only some fields within the structure. A new structure with only the selected fields
///    will be used in place of the original value.
///
/// *NOTE: the filters and fragments are not yet supported in current zenoh version.*
#[allow(non_camel_case_types)]
#[pyclass]
pub struct Selector {
    pub(crate) s: ZSelector<'static>,
}

#[allow(non_snake_case)]
#[pymethods]
impl Selector {
    /// The part of this selector identifying which keys should be part of the selection.
    /// I.e. all characters before `?`.
    ///
    /// :type: :class:`KeyExpr`
    #[getter]
    fn key_selector(&self) -> KeyExpr {
        KeyExpr {
            inner: self.s.key_selector.to_owned(),
        }
    }

    /// the part of this selector identifying which values should be part of the selection.
    /// I.e. all characters starting from `?`.
    ///
    /// :type: str
    #[getter]
    fn value_selector(&self) -> &str {
        self.s.value_selector.as_ref()
    }

    /// Parses the `value_selector` part of this `Selector`.
    ///
    /// :rtype: :class:`ValueSelector`
    fn parse_value_selector(&self) -> PyResult<ValueSelector> {
        let zvs = self.s.parse_value_selector().map_err(to_pyerr)?;
        Ok(ValueSelector {
            filter: zvs.filter.to_owned(),
            properties: zvs.properties.0,
            fragment: zvs.fragment.map(|cow| cow.to_owned()),
        })
    }
}

#[pyproto]
impl PyObjectProtocol for Selector {
    fn __str__(&self) -> String {
        self.s.to_string()
    }
}

impl From<Selector> for ZSelector<'static> {
    fn from(s: Selector) -> ZSelector<'static> {
        s.s
    }
}

impl From<ZSelector<'static>> for Selector {
    fn from(s: ZSelector<'static>) -> Selector {
        Selector { s }
    }
}

/// A class that can be used to help decoding or encoding the `value_selector` part of a :class:`Selector`.
#[allow(non_camel_case_types)]
#[pyclass]
pub struct ValueSelector {
    pub(crate) filter: String,
    pub(crate) properties: HashMap<String, String>,
    pub(crate) fragment: Option<String>,
}

#[allow(non_snake_case)]
#[pymethods]
impl ValueSelector {
    /// the filter part of this `ValueSelector`, if any (all characters after `?` and before `(` or `[`)
    ///
    /// :type: str
    #[getter]
    fn filter(&self) -> &str {
        self.filter.as_ref()
    }

    /// the properties part of this `ValueSelector`) (all characters between ``( )`` and after `?`)
    ///
    /// :type: str
    #[getter]
    fn properties(&self) -> HashMap<String, String> {
        self.properties.clone()
    }

    /// the filter part of this `ValueSelector`, if any (all characters after `?` and before `(` or `[`)
    ///
    /// :type: str
    #[getter]
    fn fragment(&self) -> Option<&str> {
        self.fragment.as_ref().map(|s| s.as_ref())
    }
}

/// A Zenoh id
#[pyclass]
pub(crate) struct ZenohId {
    pub(crate) p: zenoh::prelude::ZenohId,
}

#[pyproto]
impl PyObjectProtocol for ZenohId {
    fn __str__(&self) -> String {
        self.p.to_string()
    }
}

/// A zenoh Value, consisting of a payload (bytes) and an :class:`Encoding`.
///
/// It can be created directly from the supported primitive types.
/// The value is automatically encoded in the payload and the Encoding is set accordingly.
///
/// Or it can be created from a tuple **(payload, encoding)**, where:
///
///  - payload has type **bytes** or **str** (the string is automatically converted into bytes)
///  - encoding has type :class:`Encoding`
///
/// :Examples:
///
/// >>> import json, zenoh
/// >>> from zenoh import Encoding, Value
/// >>>
/// >>> string_value = Value('Hello World!')
/// >>> int_value = Value(42)
/// >>> float_value = Value(3.14)
/// >>> bytes_value = Value(b'\x48\x69\x21')
/// >>> properties_value = Value({'p1': 'v1', 'p2': 'v2'})
/// >>>
/// >>> json_value = Value((json.dumps(['foo', {'bar': ('baz', None, 1.0, 2)}]), Encoding.TEXT_JSON))
/// >>> xml_value = Value(('<foo>bar</foo>', Encoding.TEXT_XML))
/// >>> custom_value = Value((b'\x48\x69\x21', Encoding.APP_CUSTOM.with_suffix('my_encoding')))
#[pyclass]
#[derive(Clone)]
pub struct Value {
    pub(crate) v: ZValue,
}
impl From<Value> for ZValue {
    fn from(v: Value) -> Self {
        v.v
    }
}
impl From<ZValue> for Value {
    fn from(v: ZValue) -> Self {
        Value { v }
    }
}

trait IntoPyAlt<U> {
    fn into_py_alt(self, py: Python) -> U;
}

impl IntoPyAlt<PyObject> for serde_json::Value {
    fn into_py_alt(self, py: Python) -> PyObject {
        match self {
            serde_json::Value::Null => py.None(),
            serde_json::Value::Bool(v) => v.into_py(py),
            serde_json::Value::Number(v) => v.into_py_alt(py),
            serde_json::Value::String(v) => v.into_py(py),
            serde_json::Value::Array(a) => a
                .into_iter()
                .map(|v| v.into_py_alt(py))
                .collect::<Vec<_>>()
                .into_py(py),
            serde_json::Value::Object(m) => m
                .into_iter()
                .map(|(k, v)| (k, v.into_py_alt(py)))
                .collect::<std::collections::HashMap<_, _>>()
                .into_py(py),
        }
    }
}
impl IntoPyAlt<PyObject> for serde_json::Number {
    fn into_py_alt(self, py: Python) -> PyObject {
        if let Some(v) = self.as_u64() {
            return v.into_py(py);
        }
        if let Some(v) = self.as_i64() {
            return v.into_py(py);
        }
        if let Some(v) = self.as_f64() {
            return v.into_py(py);
        }
        unreachable!()
    }
}

fn decode_value(value: &ZValue, py: Python) -> PyResult<PyObject> {
    match value.encoding.prefix() {
        ZKnownEncoding::Empty | ZKnownEncoding::AppOctetStream => {
            Ok(value.payload.contiguous().into_py(py))
        }
        ZKnownEncoding::TextPlain => {
            Ok(String::from_utf8_lossy(&value.payload.contiguous()).into_py(py))
        }
        ZKnownEncoding::AppProperties => zenoh::properties::Properties::try_from(value)
            .map(|v| v.0.into_py(py))
            .map_err(|e| exceptions::PyTypeError::new_err(e.to_string())),
        ZKnownEncoding::AppJson | ZKnownEncoding::TextJson => serde_json::Value::try_from(value)
            .map(|v| v.into_py_alt(py))
            .map_err(|e| exceptions::PyTypeError::new_err(e.to_string())),
        ZKnownEncoding::AppInteger => i64::try_from(value)
            .map(|v| v.into_py(py))
            .map_err(|e| exceptions::PyTypeError::new_err(e.to_string())),
        ZKnownEncoding::AppFloat => f64::try_from(value)
            .map(|v| v.into_py(py))
            .map_err(|e| exceptions::PyTypeError::new_err(e.to_string())),
        _ => Err(exceptions::PyTypeError::new_err(format!(
            "Don't know how to decode Value's payload with encoding: {}",
            value.encoding
        ))),
    }
}

#[allow(non_snake_case)]
#[pymethods]
impl Value {
    #[new]
    fn new(any: &PyAny) -> PyResult<Self> {
        Ok(Value {
            v: zvalue_of_pyany(any)?,
        })
    }

    /// the payload the Value.
    ///
    /// :type: **bytes**
    #[getter]
    fn payload<'a>(&self, py: Python<'a>) -> &'a PyBytes {
        PyBytes::new(py, self.v.payload.contiguous().as_ref())
    }

    /// the encoding of the Value.
    ///
    /// :type: :class:`Encoding`
    #[getter]
    fn encoding(&self) -> PyResult<Encoding> {
        Ok(self.v.encoding.clone().into())
    }

    /// Try to decode the value's payload according to it's encoding, and return a typed object or primitive.
    ///
    /// :rtype: depend on the encoding flag (e.g. str for a StringUtf8 Value, int for an Integer Value ...)
    fn decode(&self, py: Python) -> PyResult<PyObject> {
        decode_value(&self.v, py)
    }
}

#[pyproto]
impl PyObjectProtocol for Value {
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self.v))
    }

    fn __repr__(&self) -> PyResult<String> {
        self.__str__()
    }
}

pub(crate) fn zvalue_of_pyany(obj: &PyAny) -> PyResult<ZValue> {
    match obj.get_type().name()? {
        "Value" => {
            let v: Value = obj.extract()?;
            Ok(v.v)
        }
        "bytes" => {
            let buf: &[u8] = obj.extract()?;
            Ok(Vec::from(buf).into())
        }
        "str" => {
            let s: String = obj.extract()?;
            Ok(s.into())
        }
        "dict" => {
            let props: HashMap<String, String> = obj.extract()?;
            Ok(zenoh::prelude::Properties::from(props).into())
        }
        "int" => {
            let i: i64 = obj.extract()?;
            Ok(i.into())
        }
        "float" => {
            let f: f64 = obj.extract()?;
            Ok(f.into())
        }
        "tuple" => {
            let tuple: &PyTuple = obj.downcast()?;
            if tuple.len() == 2
                && (tuple.get_item(0)?.get_type().name()? == "bytes"
                    || tuple.get_item(0)?.get_type().name()? == "str")
                && (tuple.get_item(1)?.get_type().name()? == "str"
                    || tuple.get_item(1)?.get_type().name()? == "Encoding")
            {
                let buf: &[u8] = if tuple.get_item(0)?.get_type().name()? == "bytes" {
                    tuple.get_item(0)?.extract()?
                } else {
                    tuple.get_item(0)?.extract::<&str>()?.as_bytes()
                };
                let encoding_descr: ZEncoding = if tuple.get_item(1)?.get_type().name()? == "str" {
                    tuple.get_item(1)?.extract::<String>()?.into()
                } else {
                    tuple.get_item(1)?.extract::<Encoding>()?.e
                };
                Ok(ZValue::new(Vec::from(buf).into()).encoding(encoding_descr))
            } else {
                Err(PyErr::new::<exceptions::PyValueError, _>(format!(
                    "Cannot convert type '{:?}' to a zenoh Value",
                    tuple
                )))
            }
        }
        x => Err(PyErr::new::<exceptions::PyValueError, _>(format!(
            "Cannot convert type '{}' to a zenoh Value",
            x
        ))),
    }
}

/// A Timestamp composed of a time and the identifier of the timestamp source.
#[pyclass]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Timestamp {
    pub(crate) t: zenoh::time::Timestamp,
}

#[pymethods]
impl Timestamp {
    /// The time in seconds since the UNIX EPOCH (January 1, 1970, 00:00:00 (UTC))
    /// as a floating point number.
    ///
    /// :type: **float**
    #[getter]
    fn time(&self) -> f64 {
        self.t.get_time().to_duration().as_secs_f64()
    }

    /// The identifier of the timestamp source
    ///
    /// :type: **bytes**
    #[getter]
    fn id(&self) -> &[u8] {
        self.t.get_id().as_slice()
    }
}

#[pyproto]
impl PyObjectProtocol for Timestamp {
    fn __str__(&self) -> String {
        self.t.to_string()
    }
}

/// Some informations about the associated data
#[pyclass]
#[derive(Clone)]
pub(crate) struct SourceInfo {
    pub(crate) i: zenoh::prelude::SourceInfo,
}

#[pymethods]
impl SourceInfo {
    /// The :class:`ZenohId` of the data source.
    ///
    /// :type: :class:`ZenohId` or `None`
    #[getter]
    fn source_id(&self) -> Option<ZenohId> {
        self.i.source_id.as_ref().map(|p| ZenohId { p: *p })
    }

    /// The source sequence number of the data.
    ///
    /// :type: int or `None`
    #[getter]
    fn source_sn(&self) -> Option<ZInt> {
        self.i.source_sn
    }

    /// The :class:`ZenohId` of the 1st router that routed the data.
    ///
    /// :type: :class:`ZenohId` or `None`
    #[getter]
    fn first_router_id(&self) -> Option<ZenohId> {
        self.i.first_router_id.as_ref().map(|p| ZenohId { p: *p })
    }

    /// The first router sequence number of the data.
    ///
    /// :type: int or `None`
    #[getter]
    fn first_router_sn(&self) -> Option<ZInt> {
        self.i.first_router_sn
    }
}

/// A zenoh sample. It can be constructed with:
///
/// :param key_expr: The key expression of this Sample
/// :type key_expr: a :class:`KeyExpr` or any type convertible to a :class:`KeyExpr`
///                 (see its constructor's accepted parameters)
/// :param value: The value of this Sample
/// :type value: any type convertible to a :class:`Value` (see below)
/// :param \**kwargs:
///    See below
///
/// :Keyword Arguments:
///    * **timestamp** (:class:`Timestamp`) --
///      The Timestamp of this Sample
///    * **source_info** (:class:`SourceInfo`) --
///      Infos on the source of this Sample.
#[pyclass]
#[pyo3(text_signature = "(key_expr, payload, timestamp=None, source_info=None)")]
#[derive(Clone)]
pub(crate) struct Sample {
    pub(crate) s: ZSample,
}

impl pyo3::conversion::ToPyObject for Sample {
    fn to_object(&self, py: Python) -> pyo3::PyObject {
        pyo3::IntoPy::into_py(pyo3::Py::new(py, self.clone()).unwrap(), py)
    }
}

#[pymethods]
impl Sample {
    #[new]
    #[args(kwargs = "**")]
    fn new(key_expr: &PyAny, value: &PyAny, kwargs: Option<&PyDict>) -> PyResult<Self> {
        let key_expr = zkey_expr_of_pyany(key_expr).unwrap();
        let value = zvalue_of_pyany(value).unwrap();
        let mut s = zenoh::prelude::Sample::new(key_expr.to_owned(), value);
        if let Some(kwargs) = kwargs {
            if let Some(arg) = kwargs.get_item("timestamp") {
                s = s.with_timestamp(arg.extract::<Timestamp>()?.t);
            }
            if let Some(arg) = kwargs.get_item("source_info") {
                s = s.with_source_info(arg.extract::<SourceInfo>()?.i);
            }
        }
        Ok(Sample { s })
    }

    /// The key expression
    ///
    /// :type: :class:`KeyExpr`
    #[getter]
    fn key_expr(&self) -> KeyExpr {
        self.s.key_expr.to_owned().into()
    }

    /// The value
    ///
    /// :type: :class:`Value`
    #[getter]
    fn value(&self) -> Value {
        Value {
            v: self.s.value.clone(),
        }
    }

    /// The data payload
    ///
    /// :type: :class:`SampleKind`
    #[getter]
    fn kind(&self) -> SampleKind {
        self.s.kind.into()
    }

    /// Some information about the data
    ///
    /// :type: :class:`SourceInfo` or `None`
    #[getter]
    fn source_info(&self) -> Option<SourceInfo> {
        Some(SourceInfo {
            i: self.s.source_info.clone(),
        })
    }

    /// The timestamp
    ///
    /// :type: :class:`Timestamp` or `None`
    #[getter]
    fn timestamp(&self) -> Option<Timestamp> {
        self.s.timestamp.map(|t| Timestamp { t })
    }

    /// the payload of the Value of this Sample.
    ///
    /// This is a shortcut for sample.value.payload.
    ///
    /// :type: **bytes**
    #[getter]
    fn payload<'a>(&self, py: Python<'a>) -> &'a PyBytes {
        PyBytes::new(py, self.s.value.payload.contiguous().as_ref())
    }

    /// the encoding of the Value of this Sample.
    ///
    /// This is a shortcut for sample.value.encoding.
    ///
    /// :type: :class:`Encoding`
    #[getter]
    fn encoding(&self) -> PyResult<Encoding> {
        Ok(self.s.value.encoding.clone().into())
    }

    /// Try to decode the value's payload of this Sample according to it's encoding, and return a typed object or primitive.
    ///
    /// This is a shortcut for sample.value.decode().
    ///
    /// :rtype: depend on the encoding flag (e.g. str for a StringUtf8 Value, int for an Integer Value ...)
    fn decode(&self, py: Python) -> PyResult<PyObject> {
        decode_value(&self.s.value, py)
    }
}

#[pyproto]
impl PyObjectProtocol for Sample {
    fn __str__(&self) -> String {
        format!("{:?}", self.s)
    }

    fn __repr__(&self) -> String {
        self.__str__()
    }
}

/// An error resulting from a get operation. The error
/// can either be a user error returned by a queryable
/// or an infrastructure error.
///
/// It can be constructed with:
///
/// :param value: The value of this Error
/// :type value: any type convertible to a :class:`Value`
#[pyclass]
#[pyo3(text_signature = "(value)")]
#[derive(Clone)]
pub(crate) struct Error {
    pub(crate) v: zenoh::prelude::Value,
}

impl pyo3::conversion::ToPyObject for Error {
    fn to_object(&self, py: Python) -> pyo3::PyObject {
        pyo3::IntoPy::into_py(pyo3::Py::new(py, self.clone()).unwrap(), py)
    }
}

#[pymethods]
impl Error {
    #[new]
    fn new(value: &PyAny) -> PyResult<Self> {
        let v = zvalue_of_pyany(value).unwrap();
        Ok(Error { v })
    }

    /// The Value of this Error
    ///
    /// :type: Value
    #[getter]
    fn value(&self) -> Value {
        Value { v: self.v.clone() }
    }

    /// the payload of the Value of this Error.
    ///
    /// :type: **bytes**
    #[getter]
    fn payload<'a>(&self, py: Python<'a>) -> &'a PyBytes {
        PyBytes::new(py, self.v.payload.contiguous().as_ref())
    }

    /// the encoding of the Value of this Error.
    ///
    /// :type: :class:`Encoding`
    #[getter]
    fn encoding(&self) -> PyResult<Encoding> {
        Ok(self.v.encoding.clone().into())
    }

    /// Try to decode this Error value's payload according to it's encoding, and return a typed object or primitive.
    ///
    /// :rtype: depend on the encoding flag (e.g. str for a StringUtf8 Value, int for an Integer Value ...)
    fn decode(&self, py: Python) -> PyResult<PyObject> {
        decode_value(&self.v, py)
    }
}

#[pyproto]
impl PyObjectProtocol for Error {
    fn __str__(&self) -> String {
        format!("{:?}", self.v)
    }

    fn __repr__(&self) -> String {
        self.__str__()
    }
}

#[derive(FromPyObject, Clone)]
enum Result {
    #[pyo3(transparent)]
    Ok(Sample),
    #[pyo3(transparent)]
    Err(Error),
}

impl IntoPy<PyObject> for Result {
    fn into_py(self, py: Python) -> pyo3::PyObject {
        match self {
            Result::Ok(s) => s.into_py(py),
            Result::Err(e) => e.into_py(py),
        }
    }
}

impl From<std::result::Result<ZSample, ZValue>> for Result {
    fn from(r: std::result::Result<ZSample, ZValue>) -> Self {
        match r {
            Ok(s) => Result::Ok(Sample { s }),
            Err(v) => Result::Err(Error { v }),
        }
    }
}

impl From<Result> for std::result::Result<ZSample, ZValue> {
    fn from(r: Result) -> Self {
        match r {
            Result::Ok(s) => std::result::Result::Ok(s.s),
            Result::Err(v) => std::result::Result::Err(v.v),
        }
    }
}

// zenoh.Reliability (simulate the enum as a class with static methods for the cases,
// waiting for https://github.com/PyO3/pyo3/issues/834 to be fixed)
//
/// The kind of reliability
#[pyclass]
#[derive(Clone, Copy, PartialEq, Default)]
pub(crate) struct Reliability {
    pub(crate) r: zenoh::subscriber::Reliability,
}

#[allow(non_snake_case)]
#[pymethods]
impl Reliability {
    #[classattr]
    fn BestEffort() -> Reliability {
        Reliability {
            r: zenoh::subscriber::Reliability::BestEffort,
        }
    }

    #[classattr]
    fn Reliable() -> Reliability {
        Reliability {
            r: zenoh::subscriber::Reliability::Reliable,
        }
    }
}

// zenoh.SubMode (simulate the enum as a class with static methods for the cases,
// waiting for https://github.com/PyO3/pyo3/issues/834 to be fixed)
//
/// The subscription mode.
#[pyclass]
#[derive(Clone, Default)]
pub(crate) struct SubMode {
    pub(crate) m: zenoh::subscriber::SubMode,
}

#[allow(non_snake_case)]
#[pymethods]
impl SubMode {
    #[classattr]
    fn Push() -> SubMode {
        SubMode {
            m: zenoh::subscriber::SubMode::Push,
        }
    }

    #[classattr]
    fn Pull() -> SubMode {
        SubMode {
            m: zenoh::subscriber::SubMode::Pull,
        }
    }
}

/// A time period.
#[pyclass]
#[pyo3(text_signature = "(origin, period, duration)")]
#[derive(Clone)]
pub(crate) struct Period {
    pub(crate) p: zenoh::time::Period,
}

#[pymethods]
impl Period {
    #[new]
    fn new(origin: ZInt, period: ZInt, duration: ZInt) -> Period {
        Period {
            p: zenoh::time::Period {
                origin,
                period,
                duration,
            },
        }
    }
}

/// A subscriber
#[pyclass]
pub(crate) struct Subscriber {
    pub(crate) inner: Option<CallbackSubscriber<'static>>,
}

#[pymethods]
impl Subscriber {
    /// Pull available data for a pull-mode subscriber.
    fn pull(&self) -> PyResult<()> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| PyErr::new::<ZError, _>("the Subscriber was closed"))?;
        inner.pull().res().map_err(to_pyerr)
    }

    /// Close the subscriber.
    fn close(&mut self) -> PyResult<()> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| PyErr::new::<ZError, _>("the Subscriber was already closed"))?;
        inner.close().res().map_err(to_pyerr)
    }
}

/// Type received by a queryable callback. See :meth:`Session.queryable`.
#[pyclass]
#[derive(Clone)]
pub(crate) struct Query {
    pub(crate) q: async_std::sync::Arc<zenoh::queryable::Query>,
}

impl pyo3::conversion::ToPyObject for Query {
    fn to_object(&self, py: Python) -> pyo3::PyObject {
        pyo3::IntoPy::into_py(pyo3::Py::new(py, self.clone()).unwrap(), py)
    }
}

#[pymethods]
impl Query {
    /// The full :class:`Selector` of this Query.
    ///
    /// :type: :class:`Selector`
    #[getter]
    fn selector(&self) -> Selector {
        self.q.selector().to_owned().into()
    }

    /// The key selector part of this Query.
    ///
    /// :type: :class:`KeyExpr`
    #[getter]
    fn key_selector(&self) -> KeyExpr {
        self.q.key_selector().to_owned().into()
    }

    /// The value selector part of this Query.
    ///
    /// :type: str
    #[getter]
    fn value_selector(&self) -> String {
        self.q.value_selector().to_string()
    }

    /// Send a reply to this query
    ///
    /// :param sample: the reply sample
    /// :type: Sample
    #[pyo3(text_signature = "(self, sample)")]
    fn reply(&self, sample: Result) {
        if let Err(e) = self.q.reply(sample.into()).res() {
            warn!("Error in Query::reply() : {}", e);
        }
    }
}

/// An entity able to reply to queries.
#[pyclass]
pub(crate) struct Queryable {
    pub(crate) inner: Option<CallbackQueryable<'static>>,
}

#[pymethods]
impl Queryable {
    /// Close the queryable.
    fn close(&mut self) -> PyResult<()> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| PyErr::new::<ZError, _>("the Queryable was already closed"))?;
        inner.close().res().map_err(to_pyerr)
    }
}

// zenoh.Target (simulate the enum as a class with static methods for the cases,
// waiting for https://github.com/PyO3/pyo3/issues/834 to be fixed)
//
/// The zenoh queryables that should be target of a :meth:`Session.get`.
#[pyclass]
#[derive(Clone)]
pub(crate) struct QueryTarget {
    pub(crate) t: zenoh::query::QueryTarget,
}

#[allow(non_snake_case)]
#[pymethods]
impl QueryTarget {
    #[staticmethod]
    fn BestMatching() -> QueryTarget {
        QueryTarget {
            t: zenoh::query::QueryTarget::BestMatching,
        }
    }

    #[cfg(features = "complete_n")]
    #[staticmethod]
    #[pyo3(text_signature = "(n)")]
    fn Complete(n: ZInt) -> QueryTarget {
        QueryTarget {
            t: zenoh::query::QueryTarget::Complete { n },
        }
    }

    #[staticmethod]
    fn All() -> QueryTarget {
        QueryTarget {
            t: zenoh::query::QueryTarget::All,
        }
    }

    #[staticmethod]
    fn AllComplete() -> QueryTarget {
        QueryTarget {
            t: zenoh::query::QueryTarget::AllComplete,
        }
    }

    #[staticmethod]
    fn No() -> QueryTarget {
        QueryTarget {
            t: zenoh::query::QueryTarget::None,
        }
    }
}

// zenoh.ConsolidationMode (simulate the enum as a class with static methods for the cases,
// waiting for https://github.com/PyO3/pyo3/issues/834 to be fixed)
//
/// The kind of consolidation that should be applied on replies to a :meth:`Session.get`.
#[pyclass]
#[derive(Clone)]
pub(crate) struct ConsolidationMode {
    pub(crate) c: zenoh::query::ConsolidationMode,
}

#[allow(non_snake_case)]
#[pymethods]
impl ConsolidationMode {
    #[classattr]
    fn No() -> ConsolidationMode {
        ConsolidationMode {
            c: zenoh::query::ConsolidationMode::None,
        }
    }

    #[classattr]
    fn Lazy() -> ConsolidationMode {
        ConsolidationMode {
            c: zenoh::query::ConsolidationMode::Lazy,
        }
    }

    #[classattr]
    fn Full() -> ConsolidationMode {
        ConsolidationMode {
            c: zenoh::query::ConsolidationMode::Full,
        }
    }
}

// zenoh.ConsolidationStrategy
//
/// The kind of consolidation that should be applied on replies to a :meth:`Session.get`
/// at the different stages of the reply process.
///
/// :param first_routers: the consolidation mode to apply on first routers of the replies routing path (default: :attr:`ConsolidationMode.Lazy`)
/// :type first_routers: ConsolidationMode, optional
/// :param last_router: the consolidation mode to apply on last router of the replies routing path (default: :attr:`ConsolidationMode.Lazy`)
/// :type last_router: ConsolidationMode, optional
/// :param reception: the consolidation mode to apply at reception of the replies (default: :attr:`ConsolidationMode.Full`)
/// :type reception: ConsolidationMode, optional
#[pyclass]
#[pyo3(text_signature = "(first_routers=None, last_router=None, reception=None)")]
#[derive(Clone, Default)]
pub(crate) struct ConsolidationStrategy {
    pub(crate) c: zenoh::query::ConsolidationStrategy,
}

#[pymethods]
impl ConsolidationStrategy {
    #[new]
    fn new(
        first_routers: Option<ConsolidationMode>,
        last_router: Option<ConsolidationMode>,
        reception: Option<ConsolidationMode>,
    ) -> ConsolidationStrategy {
        let mut c = zenoh::query::ConsolidationStrategy::default();
        if let Some(f) = first_routers {
            c.first_routers = f.c;
        }
        if let Some(l) = last_router {
            c.last_router = l.c;
        }
        if let Some(r) = reception {
            c.reception = r.c;
        }
        ConsolidationStrategy { c }
    }

    /// No consolidation performed.
    ///
    /// This is usefull when querying timeseries data bases or
    /// when using quorums.
    #[staticmethod]
    fn none() -> Self {
        Self {
            c: zenoh::query::ConsolidationStrategy::none(),
        }
    }

    /// Lazy consolidation performed at all stages.
    ///
    /// This strategy offers the best latency. Replies are directly
    /// transmitted to the application when received without needing
    /// to wait for all replies.
    ///
    /// This mode does not garantie that there will be no duplicates.
    #[staticmethod]
    pub fn lazy() -> Self {
        Self {
            c: zenoh::query::ConsolidationStrategy::lazy(),
        }
    }

    /// Full consolidation performed at reception.
    ///
    /// This is the default strategy. It offers the best latency while
    /// garantying that there will be no duplicates.
    #[staticmethod]
    pub fn reception() -> Self {
        Self {
            c: zenoh::query::ConsolidationStrategy::reception(),
        }
    }

    /// Full consolidation performed on last router and at reception.
    ///
    /// This mode offers a good latency while optimizing bandwidth on
    /// the last transport link between the router and the application.
    #[staticmethod]
    pub fn last_router() -> Self {
        Self {
            c: zenoh::query::ConsolidationStrategy::last_router(),
        }
    }

    /// Full consolidation performed everywhere.
    ///
    /// This mode optimizes bandwidth on all links in the system
    /// but will provide a very poor latency.
    #[staticmethod]
    pub fn full() -> Self {
        Self {
            c: zenoh::query::ConsolidationStrategy::full(),
        }
    }
}

// zenoh.QueryConsolidation (simulate the enum as a class with static methods for the cases,
// waiting for https://github.com/PyO3/pyo3/issues/834 to be fixed)
//
/// The replies consolidation strategy to apply on replies to a :meth:`Session.get`.
#[pyclass]
#[pyo3(text_signature = "(first_routers=None, last_router=None, reception=None)")]
#[derive(Clone, Default)]
pub(crate) struct QueryConsolidation {
    pub(crate) c: zenoh::query::QueryConsolidation,
}

#[allow(non_snake_case)]
#[pymethods]
impl QueryConsolidation {
    /// Automatic query consolidation strategy selection.
    ///
    /// A query consolidation strategy will automatically be selected depending
    /// the query selector. If the selector contains time range properties,
    /// no consolidation is performed. Otherwise the reception strategy is used.
    #[staticmethod]
    fn Auto() -> Self {
        QueryConsolidation {
            c: zenoh::query::QueryConsolidation::Auto,
        }
    }

    /// User defined query consolidation strategy.
    #[staticmethod]
    fn Manual(strategy: ConsolidationStrategy) -> Self {
        QueryConsolidation {
            c: zenoh::query::QueryConsolidation::Manual(strategy.c),
        }
    }

    /// No consolidation performed.
    ///
    /// This is usefull when querying timeseries data bases or
    /// when using quorums.
    #[staticmethod]
    fn none() -> Self {
        Self {
            c: zenoh::query::QueryConsolidation::none(),
        }
    }

    /// Lazy consolidation performed at all stages.
    ///
    /// This strategy offers the best latency. Replies are directly
    /// transmitted to the application when received without needing
    /// to wait for all replies.
    ///
    /// This mode does not garantie that there will be no duplicates.
    #[staticmethod]
    pub fn lazy() -> Self {
        Self {
            c: zenoh::query::QueryConsolidation::lazy(),
        }
    }

    /// Full consolidation performed at reception.
    ///
    /// This is the default strategy. It offers the best latency while
    /// garantying that there will be no duplicates.
    #[staticmethod]
    pub fn reception() -> Self {
        Self {
            c: zenoh::query::QueryConsolidation::reception(),
        }
    }

    /// Full consolidation performed on last router and at reception.
    ///
    /// This mode offers a good latency while optimizing bandwidth on
    /// the last transport link between the router and the application.
    #[staticmethod]
    pub fn last_router() -> Self {
        Self {
            c: zenoh::query::QueryConsolidation::last_router(),
        }
    }

    /// Full consolidation performed everywhere.
    ///
    /// This mode optimizes bandwidth on all links in the system
    /// but will provide a very poor latency.
    #[staticmethod]
    pub fn full() -> Self {
        Self {
            c: zenoh::query::QueryConsolidation::full(),
        }
    }
}

/// Type received by a get callback. See :meth:`Session.get`.
#[pyclass]
#[derive(Clone)]
pub(crate) struct Reply {
    pub(crate) r: zenoh::query::Reply,
}

impl pyo3::conversion::ToPyObject for Reply {
    fn to_object(&self, py: Python) -> pyo3::PyObject {
        pyo3::IntoPy::into_py(pyo3::Py::new(py, self.clone()).unwrap(), py)
    }
}

#[pymethods]
impl Reply {
    /// The sample
    ///
    /// :type: Sample
    #[getter]
    fn sample(&self) -> Result {
        self.r.sample.clone().into()
    }

    /// The identifier of reply source
    ///
    /// :type: ZenohId
    fn replier_id(&self) -> ZenohId {
        ZenohId {
            p: self.r.replier_id,
        }
    }
}

// zenoh.CongestionControl (simulate the enum as a class with static methods for the cases,
// waiting for https://github.com/PyO3/pyo3/issues/834 to be fixed)
//
/// The kind of congestion control.
#[pyclass]
#[derive(Clone, Default)]
pub struct CongestionControl {
    pub(crate) cc: zenoh::publication::CongestionControl,
}

#[allow(non_snake_case)]
#[pymethods]
impl CongestionControl {
    #[classattr]
    fn Drop() -> CongestionControl {
        CongestionControl {
            cc: zenoh::publication::CongestionControl::Drop,
        }
    }
    #[classattr]
    fn Block() -> CongestionControl {
        CongestionControl {
            cc: zenoh::publication::CongestionControl::Block,
        }
    }
}

#[pyclass]
#[derive(Clone, Default)]
pub struct Priority {
    pub(crate) p: zenoh::prelude::Priority,
}

#[allow(non_snake_case)]
#[pymethods]
impl Priority {
    #[classattr]
    fn Background() -> Self {
        Priority {
            p: zenoh::prelude::Priority::Background,
        }
    }
    #[classattr]
    fn Data() -> Self {
        Priority {
            p: zenoh::prelude::Priority::Data,
        }
    }
    #[classattr]
    fn DataHigh() -> Self {
        Priority {
            p: zenoh::prelude::Priority::DataHigh,
        }
    }
    #[classattr]
    fn DataLow() -> Self {
        Priority {
            p: zenoh::prelude::Priority::DataLow,
        }
    }
    #[classattr]
    fn InteractiveHigh() -> Self {
        Priority {
            p: zenoh::prelude::Priority::InteractiveHigh,
        }
    }
    #[classattr]
    fn InteractiveLow() -> Self {
        Priority {
            p: zenoh::prelude::Priority::InteractiveLow,
        }
    }
    #[classattr]
    fn RealTime() -> Self {
        Priority {
            p: zenoh::prelude::Priority::RealTime,
        }
    }
}
