//! Intermediate representation of item data.

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
	punctuated::Punctuated, spanned::Spanned, Attribute, Ident, Meta, Result, Token, Variant,
};

use crate::{Data, Error, Incomparable, Trait};

/// Fields or variants of an item.
#[cfg_attr(test, derive(Debug))]
#[allow(clippy::large_enum_variant)]
pub enum Item<'a> {
	/// Enum.
	Enum {
		/// Type of discriminant used.
		discriminant: Discriminant,
		/// [`struct@Ident`] of this enum.
		ident: &'a Ident,
		/// [`Incomparable`] attribute of this enum.
		incomparable: Incomparable,
		/// Variants of this enum.
		variants: Vec<Data<'a>>,
	},
	/// Struct, tuple struct or union.
	Item(Data<'a>),
}

impl Item<'_> {
	/// Returns [`struct@Ident`] of this [`Item`].
	pub fn ident(&self) -> &Ident {
		match self {
			Item::Item(data) => data.ident,
			Item::Enum { ident, .. } => ident,
		}
	}

	/// Returns `true` if this [`Item`] if an enum.
	pub fn is_enum(&self) -> bool {
		match self {
			Item::Enum { .. } => true,
			Item::Item(_) => false,
		}
	}

	/// Returns `true` if any field is skipped with that [`Trait`].
	pub fn any_skip_trait(&self, trait_: Trait) -> bool {
		match self {
			Item::Item(data) => data.any_skip_trait(trait_),
			Item::Enum { variants, .. } => variants.iter().any(|data| data.any_skip_trait(trait_)),
		}
	}

	/// Returns `true` if any field uses `Zeroize(fqs)`.
	#[cfg(feature = "zeroize")]
	pub fn any_fqs(&self) -> bool {
		use crate::Either;

		match self {
			Item::Item(data) => match data.fields() {
				Either::Left(fields) => fields.fields.iter().any(|field| field.attr.zeroize_fqs.0),
				Either::Right(_) => false,
			},
			Item::Enum { variants, .. } => variants.iter().any(|data| match data.fields() {
				Either::Left(fields) => fields.fields.iter().any(|field| field.attr.zeroize_fqs.0),
				Either::Right(_) => false,
			}),
		}
	}

	/// Returns `true` if all [`Fields`](crate::data::Fields) are empty for this
	/// [`Trait`].
	pub fn is_empty(&self, trait_: Trait) -> bool {
		match self {
			Item::Enum { variants, .. } => variants.iter().all(|data| data.is_empty(trait_)),
			Item::Item(data) => data.is_empty(trait_),
		}
	}

	/// Returns `true` if the item is incomparable or all (≥1) variants are
	/// incomparable.
	pub fn is_incomparable(&self) -> bool {
		match self {
			Item::Enum {
				variants,
				incomparable,
				..
			} => {
				incomparable.0.is_some()
					|| !variants.is_empty() && variants.iter().all(Data::is_incomparable)
			}
			Item::Item(data) => data.is_incomparable(),
		}
	}
}

/// Type of discriminant used.
#[cfg_attr(test, derive(Debug))]
pub enum Discriminant {
	/// The enum uses the default representation but has a non-unit variant or
	/// an enum with a C representation without an integer representation.
	Unknown,
	/// The enum has only a single variant.
	Single,
	/// The enum uses the default representation and has only unit variants.
	UnitDefault,
	/// The enum uses a non-default representation and has only unit variants.
	UnitRepr(Representation),
	/// The enum uses a non-default representation and has a non-unit variant.
	Repr(Representation),
}

impl Discriminant {
	/// Parse the representation of an item.
	pub fn parse(attrs: &[Attribute], variants: &Punctuated<Variant, Token![,]>) -> Result<Self> {
		if variants.len() == 1 {
			return Ok(Self::Single);
		}

		let mut has_repr = None;
		let mut is_c = false;

		for attr in attrs {
			if attr.path().is_ident("repr") {
				if let Meta::List(list) = &attr.meta {
					let list =
						list.parse_args_with(Punctuated::<Ident, Token![,]>::parse_terminated)?;

					for ident in list {
						if ident == "C" {
							is_c = true;
						} else if let Some(repr) = Representation::parse(&ident) {
							has_repr = Some(repr);
							break;
						}
					}
				} else {
					return Err(Error::repr(attr.span()));
				}
			}
		}

		let is_unit = variants.iter().all(|variant| variant.fields.is_empty());

		Ok(if let Some(repr) = has_repr {
			if is_unit {
				Self::UnitRepr(repr)
			} else {
				Self::Repr(repr)
			}
		} else if is_unit && !is_c {
			Self::UnitDefault
		} else {
			Self::Unknown
		})
	}
}

/// The type used to represent an enum.
#[cfg_attr(test, derive(Debug))]
pub enum Representation {
	/// [`u8`].
	U8,
	/// [`u16`].
	U16,
	/// [`u32`].
	U32,
	/// [`u64`].
	U64,
	/// [`u128`].
	U128,
	/// [`usize`].
	USize,
	/// [`i8`].
	I8,
	/// [`i16`].
	I16,
	/// [`i32`].
	I32,
	/// [`i64`].
	I64,
	/// [`i128`].
	I128,
	/// [`isize`].
	ISize,
}

impl Representation {
	/// Parse an [`struct@Ident`] to a valid representation if it is.
	fn parse(ident: &Ident) -> Option<Self> {
		Some(if ident == "u8" {
			Self::U8
		} else if ident == "u16" {
			Self::U16
		} else if ident == "u32" {
			Self::U32
		} else if ident == "u64" {
			Self::U64
		} else if ident == "u128" {
			Self::U128
		} else if ident == "usize" {
			Self::USize
		} else if ident == "i8" {
			Self::I8
		} else if ident == "i16" {
			Self::I16
		} else if ident == "i32" {
			Self::I32
		} else if ident == "i64" {
			Self::I64
		} else if ident == "i128" {
			Self::I128
		} else if ident == "isize" {
			Self::ISize
		} else {
			return None;
		})
	}
}

impl ToTokens for Representation {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Representation::U8 => tokens.extend(quote! { u8 }),
			Representation::U16 => tokens.extend(quote! { u16 }),
			Representation::U32 => tokens.extend(quote! { u32 }),
			Representation::U64 => tokens.extend(quote! { u64 }),
			Representation::U128 => tokens.extend(quote! { u128 }),
			Representation::USize => tokens.extend(quote! { usize }),
			Representation::I8 => tokens.extend(quote! { i8 }),
			Representation::I16 => tokens.extend(quote! { i16 }),
			Representation::I32 => tokens.extend(quote! { i32 }),
			Representation::I64 => tokens.extend(quote! { i64 }),
			Representation::I128 => tokens.extend(quote! { i128 }),
			Representation::ISize => tokens.extend(quote! { isize }),
		}
	}
}
