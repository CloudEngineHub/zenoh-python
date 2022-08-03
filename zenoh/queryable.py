from typing import Dict

from .zenoh import _Query, _Queryable
from .keyexpr import KeyExpr, Selector
from .value import Sample

class Queryable:
	"""
	A handle to a queryable.
	
	Its main purpose is to keep the queryable active as long as it exists.

	When constructed through `Session.declare_queryable(session, keyexpr, handler)`, it exposes `handler`'s receiver
	through `self.receiver`.
	"""
	def __init__(self, inner: _Queryable, receiver):
		self._inner_ = inner
		self.receiver = receiver
	
	def undeclare(self):
		"Stops the queryable."
		self._inner_ = None

class Query(_Query):
	def __new__(cls, inner: _Query):
		return super().__new__(cls, inner)
	@property
	def key_expr(self) -> KeyExpr:
		"The query's targeted key expression"
		return KeyExpr(super().key_expr)
	@property
	def value_selector(self) -> str:
		"""
		The query's value selector.
		If you'd rather not bother with parsing it yourself, use `self.decode_value_selector()` instead.
		"""
		return super().value_selector
		
	def decode_value_selector(self) -> Dict[str, str]:
		"""
		Decodes the value selector into a dictionary.

		Raises a ZError if duplicate keys are found, as they might otherwise be used for HTTP Parameter Pollution like attacks.
		"""
		return super().decode_value_selector
	@property
	def selector(self) -> Selector:
		"""
		The query's selector as a whole.
		"""
		return Selector._upgrade_(super().selector)
	def reply(self, sample: Sample):
		"""
		Allows you to reply to a query.
		You may send any amount of replies to a single query, including 0.
		"""
		super().reply(sample)