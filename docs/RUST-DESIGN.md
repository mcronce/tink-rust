# Rust Port Design

The Rust port of Tink has the following meta-goals:

- **Diverge as little as possible from the upstream Tink code**: The Rust port is primarily based on the Go language
  version of upstream Tink, and aims to stay as close to it as possible so that future changes to Tink can be
  merged more easily. However, this does mean that some aspects of the port are not quite idiomatic Rust.
- **Don't write any crypto code**: The Rust port aims to defer all cryptographic implementations to external crates
  (currently the [RustCrypto](https://github.com/RustCrypto) crates).

The remainder of this section describes design decisions involved in the conversion from Go to Rust.

## The `Primitive` Type

The Go port uses `interface {}` to hold an arbitrary primitive, and uses [type
assertions](https://tour.golang.org/methods/15) to convert to particular primitive `interface` types.  This is not
possible in Rust, and so the Rust port includes a `Primitive` `enum` that holds all of the possible primitive types (as
trait objects):

```Rust
enum Primitive {
    Aead(Box<dyn Aead>),
    DeterministicAead(Box<dyn DeterministicAead>),
    // ...
    Verifier(Box<dyn Verifier>),
}
```

However, this has the big downside that it is impossible for third parties to extend the Rust port of Tink to include
new types of primitive without modifying the Tink source.

## The `KeyManager` Registry

A `KeyManager` is an object that handles the translation from a `Key` instance to a `Primitive` object that uses the
`Key` for its key material. Tink has a **global** registry of `KeyManager` instances, each indexed by a **type URL**
that identifies the kind of keys it supports.

This registry allows an arbitrary `Key` to be converted to a `Primitive` of the relevant type, and similarly allows
a `Keyset` to be converted to a `PrimitiveSet`.

- In Go, primitives are of type `interface {}`, and the user of the registry uses [type
  assertions](https://tour.golang.org/methods/15) to convert a general primitive to a more specific object that
  implements the `interface` of a particular primitive.
    - The global registry is automatically populated at start-of-day, by the use of
      [`init()`](https://golang.org/doc/effective_go.html#init) methods for each particular `KeyManager`
      implementation.
- In C++, the `KeyManager<P>` type is a template that is parameterized by the particular primitive
  type that it handles, so it returns primitives that are automatically type safe.  Internally, the global registry of
  key manager instances maps type URL strings to a combination of (roughly) `void *` and
  [`type_info`](https://en.cppreference.com/w/cpp/types/type_info); the particular `KeyManager<P>` is then
  recovered via `static_cast` (modulo a check that the `type_info` is sensible).
    - The global registry has to be manually populated by calling `<Primitive>Config::Register()` methods before use.
- In Rust, the `Primitive` type is an enum that encompasses all primitive types, and the user of the registry
  checks that the relevant enum variant is returned.  If all of the `Primitive`s in a `PrimitiveSet` are known to be
  of a specific primitive type, the `PrimitiveSet` can be converted to a `TypedPrimitiveSet<T>` for the relevant
  primitive type `T`.
    - The global registry has to be manually populated by calling `tink_<primitive>::init()` methods before use.

## Error Handling

Many Go functions return values of form `(ReturnType, error)`; the Rust equivalent of this is a `Result<ReturnType, E>`,
where `E` is some type that implements the [`Error` trait](https://doc.rust-lang.org/std/error/trait.Error.html).

The Rust port uses the `TinkError` type for `E`.  This type includes an optional inner `Error`, and the
`tink_core::utils` module also includes the `wrap_err()` helper, which is used as an equivalent for the common Go pattern
of wrapping errors:

```Go
x, err := library.DoSomething()
if err != nil {
	return nil, fmt.Errorf("doing something failed: %s", err)
}
```

like so:

```Rust
let x = library::do_something().map_err(|e| wrap_err("doing something failed", e))?;
```

## The `PrivateKeyManager` Type

The Go version of Tink includes a `PrivateKeyManager` interface which extends the `KeyManager` interface, and uses
down-casting type assertions to see if an instance of the latter is also an instance of the former:

```Go
	km, err := registry.GetKeyManager(privKeyData.TypeUrl)
	if err != nil {
		return nil, err
	}
	pkm, ok := km.(registry.PrivateKeyManager)
```

Rust allows a trait definition to indicate a required trait bound (`trait PrivateKeyManager: KeyManager {..}`), but does
not support down-casting; given a trait object of type `dyn KeyManager`, there is no way to determine if the object also
references a concrete type that implements the `dyn PrivateKeyManager` trait.

As a result, there is no `PrivateKeyManager` trait in the Rust port. Instead, the `KeyManager` trait includes the
`public_key_data()` method from Go's `PrivateKeyManager`, together with a `supports_private_keys()` method to allow
discovery of whether a `KeyManager` trait object supports this or not.  Both of these trait methods have default
implementations that indicate no support for private keys.

## `init` Methods

The Go port uses [`init()` functions](https://golang.org/doc/effective_go.html#init) to register primitive factories;
this is not supported in Rust, so each crate that provides a primitive has an `init()` function that should be called
before use.

## `KeyManager::new_key` Method

The Go port has a `KeyManager.NewKey` method which returns a `proto.Message` holding a new key. For the Rust port, the
equivalent `KeyManager::new_key` method returns a *serialized* protobuf message (as a `Vec<u8>`) rather than a
`prost::Message`.

This is because a returned trait object of type `dyn prost::Message` would not be of much use &ndash; almost all of the methods
on the [`prost::Message` trait](https://docs.rs/prost/0.6.1/prost/trait.Message.html) require a `self` parameter that is
[`Sized`](https://doc.rust-lang.org/std/marker/trait.Sized.html), and a bare trait object is *not* `Sized`.

## Stringly-Typed Parameters

The Go port uses [stringly-typed parameters](https://wiki.c2.com/?StringlyTyped) to indicate enumerations in various
places (e.g. hash function names, curve names).  Wherever possible, the Rust port uses strongly typed `enum`s instead:

- When the main enumeration definition is from a [protobuf
  file](https://developers.google.com/protocol-buffers/docs/proto3#enum), the generated Rust code has a corresponding
  `enum` type, but fields using that type are encoded as `i32` values.  The `enum` type is used for API parameters in
  the Rust port, and converted to `i32` values when held in a protobuf-generated `struct`.
- When enumeration values are serialized to/from JSON, their `i32` values are converted to/from string values that match
  the Go string values (see [below](#json-output)).
- Test vectors from [Wycheproof](https://github.com/google/wycheproof) use string names to identify enumeration values;
  these are converted to the relevant `enum` type in the relevant Wycheproof-driven test cases.

## JSON Output

Tink supports the encoding of `Keyset` and `EncryptedKeyset` types as JSON when the `json` feature of the `tink-core` crate
is enabled, with the following conventions:

- Values of type `bytes` are serialized to base64-encoded strings (standard encoding).
- Enum values are serialized as capitalized strings (e.g. `"ASYMMETRIC_PRIVATE"`).

The `tink_core::keyset::json_io` module includes `serde` serialization code which matches these conventions, and
the [prost-build](https://crates.io/crates/prost-build) invocation that creates the Rust protobuf message
definitions includes a collection of extra options to force the generation of the appropriate `serde`
attributes.

## Code Structure

This section describes the mapping between the upstream Go packages and the equivalent Rust crates and modules.

### Infrastructure

|  Rust Crate/Module   | Go Package |
|----------------------|------------|
| `tink_core::cryptofmt`    | `core/cryptofmt` |
| `tink_core::keyset`       | `keyset` |
| `tink_core::primitiveset` | `core/primitiveset` |
| `tink_core::registry`     | `core/registry` |
| `tink-core`               | `tink` |
| `tink-proto`         | `*_go_proto` |

### Common Crypto

|  Rust Crate/Module     | Go Package |
|------------------------|------------|
|                        | `kwp` |
| `tink_core::subtle::random` | `subtle/random` |
| `tink_core::subtle`         | `subtle` |

### Primitives

|  Rust Crate/Module   | Go Package |
|----------------------|------------|
| `tink-aead`          | `aead` |
| `tink-daead`         | `daead` |
|  TODO(#233)          | `hybrid` |
| `tink-mac`           | `mac` |
| `tink-prf`           | `prf` |
| `tink-signature`     | `signature` |
| `tink-streaming-aead`| `streamingaead` |

### Testing

|  Rust Crate/Module       | Go Package |  Notes |
|--------------------------|------------|--------|
| `tink_core::keyset::insecure` | `insecurecleartextkeyset` | Gated on (non-default) `insecure` feature |
| `tink_core::keyset::insecure` | `internal` | Gated on (non-default) `insecure` feature |
| `tink_core::keyset::insecure` | `testkeyset` | Gated on (non-default) `insecure` feature |
| `tink-tests`             | `testutil` | Depends on `insecure` feature of `tink-core` crate |
| `tink-testing`           | `services` (`/testing/go/`) |
| `tink-testing::proto`    | `testing_api_go_grpc` (`/proto/testing/`) |

### Key Management Systems

|  Rust Crate/Module   | Go Package |
|----------------------|------------|
| `tink-awskms`        | `integration/awskms` |
| `tink-gcpkms`        | `integration/gcpkms` |
|                      | `integration/hcvault` |
