mod stealth_commitments;

#[cfg(feature = "bls12_381")]
mod bls12_381_impl;
#[cfg(feature = "bn254")]
mod bn254_impl;
#[cfg(feature = "bls12_377")]
mod bls12_377_impl;

#[cfg(all(feature = "bls12_381", feature = "bls12_377", feature = "bn254"))]
compile_error!("Curves are mutually exclusive and cannot be enabled together");

#[cfg(all(feature = "bls12_377", feature = "bn254"))]
compile_error!("Curves are mutually exclusive and cannot be enabled together");

#[cfg(all(feature = "bls12_377", feature = "bls12_381"))]
compile_error!("Curves are mutually exclusive and cannot be enabled together");

#[cfg(all(feature = "bls12_381", feature = "bn254"))]
compile_error!("Curves are mutually exclusive and cannot be enabled together");

#[cfg(feature = "ffi")]
mod ffi;
