// Copyright 2020 The Tink-Rust Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
////////////////////////////////////////////////////////////////////////////////

//! Provides common methods needed in test code.

#![deny(intra_doc_link_resolution_failure)]

use std::{convert::TryInto, sync::Arc};
use tink::{
    proto::{KeyData, Keyset},
    TinkError,
};

mod constant;
pub use constant::*;
mod wycheproofutil;
pub use wycheproofutil::*;

// TODO: use tink::subtle::random helpers
use rand::{thread_rng, Rng};
pub fn get_random_bytes(size: usize) -> Vec<u8> {
    let mut data = vec![0u8; size];
    thread_rng().fill(&mut data[..]);
    data
}

/// Dummy implementation of the `KeyManager` trait.
/// It returns [`DummyAead`] when `primitive()` functions are called.
#[derive(Debug)]
pub struct DummyAeadKeyManager {
    pub type_url: String,
}

impl Default for DummyAeadKeyManager {
    fn default() -> Self {
        Self {
            type_url: AES_GCM_TYPE_URL.to_string(),
        }
    }
}

impl tink::registry::KeyManager for DummyAeadKeyManager {
    fn primitive(&self, _serialized_key: &[u8]) -> Result<tink::Primitive, TinkError> {
        Ok(tink::Primitive::Aead(Arc::new(DummyAead)))
    }

    fn new_key(&self, _serialized_key_format: &[u8]) -> Result<Vec<u8>, TinkError> {
        Err("not implemented".into())
    }

    fn does_support(&self, type_url: &str) -> bool {
        type_url == self.type_url
    }

    fn type_url(&self) -> String {
        self.type_url.to_string()
    }

    fn key_material_type(&self) -> tink::proto::key_data::KeyMaterialType {
        tink::proto::key_data::KeyMaterialType::Symmetric
    }

    fn new_key_data(&self, _serialized_key_format: &[u8]) -> Result<KeyData, TinkError> {
        Err("not implemented".into())
    }
}

/// Dummy implementation of [`tink::Aead`] trait.
#[derive(Debug)]
pub struct DummyAead;

impl tink::Aead for DummyAead {
    fn encrypt(&self, _plaintext: &[u8], _additional_data: &[u8]) -> Result<Vec<u8>, TinkError> {
        Err("dummy aead encrypt".into())
    }

    fn decrypt(&self, _ciphertext: &[u8], _additional_data: &[u8]) -> Result<Vec<u8>, TinkError> {
        Err("dummy aead decrypt".into())
    }
}

/// Dummy implementation of [`tink::Mac`] trait.
#[derive(Debug)]
pub struct DummyMac {
    pub name: String,
}

impl tink::Mac for DummyMac {
    // Computes message authentication code (MAC) for `data`.
    fn compute_mac(&self, data: &[u8]) -> Result<Vec<u8>, TinkError> {
        let mut m = Vec::new();
        m.extend_from_slice(data);
        m.extend_from_slice(self.name.as_bytes());
        Ok(m)
    }

    // Verify whether `mac` is a correct authentication code (MAC) for `data`.
    fn verify_mac(&self, _mac: &[u8], _data: &[u8]) -> Result<(), TinkError> {
        Ok(())
    }
}

/// Dummy implementation of a [`tink::registry::KmsClient`].
pub struct DummyKmsClient;

impl tink::registry::KmsClient for DummyKmsClient {
    fn supported(&self, key_uri: &str) -> bool {
        key_uri == "dummy"
    }

    fn get_aead(&self, _key_uri: &str) -> Result<Arc<dyn tink::Aead>, TinkError> {
        Ok(Arc::new(DummyAead))
    }
}

/// Create a new [`Keyset`] containing an [`AesGcmKey`](tink::proto::AesGcmKey).
pub fn new_test_aes_gcm_keyset(
    primary_output_prefix_type: tink::proto::OutputPrefixType,
) -> Keyset {
    let key_data = new_aes_gcm_key_data(16);
    new_test_keyset(key_data, primary_output_prefix_type)
}

/// Create a new [`Keyset`] containing an [`AesSivKey`](tink::proto::AesSivKey).
pub fn new_test_aes_siv_keyset(
    primary_output_prefix_type: tink::proto::OutputPrefixType,
) -> Keyset {
    // TODO: replace with dep on tink_daead
    let key_value = get_random_bytes(64);
    // let key_value = get_random_bytes(tink_daead::subtle::AES_SIV_KEY_SIZE);
    let key = &tink::proto::AesSivKey {
        version: AES_SIV_KEY_VERSION,
        key_value,
    };
    let serialized_key = proto_encode(key);
    let key_data = new_key_data(
        AES_SIV_TYPE_URL,
        &serialized_key,
        tink::proto::key_data::KeyMaterialType::Symmetric,
    );
    new_test_keyset(key_data, primary_output_prefix_type)
}

/// Create a new [`Keyset`] containing an [`HmacKey`](tink::proto::HmacKey).
pub fn new_test_hmac_keyset(
    tag_size: u32,
    primary_output_prefix_type: tink::proto::OutputPrefixType,
) -> Keyset {
    let key_data = new_hmac_key_data(tink::proto::HashType::Sha256, tag_size);
    new_test_keyset(key_data, primary_output_prefix_type)
}

/// Create a new [`Keyset`] containing an [`AesGcmHkdfKey`](tink::proto::AesGcmHkdfStreamingKey).
pub fn new_test_aes_gcm_hkdf_keyset() -> Keyset {
    const KEY_SIZE: u32 = 16;
    const DERIVED_KEY_SIZE: u32 = 16;
    const CIPHERTEXT_SEGMENT_SIZE: u32 = 4096;
    let key_data = new_aes_gcm_hkdf_key_data(
        KEY_SIZE,
        DERIVED_KEY_SIZE,
        tink::proto::HashType::Sha256,
        CIPHERTEXT_SEGMENT_SIZE,
    );
    new_test_keyset(key_data, tink::proto::OutputPrefixType::Raw)
}

/// Create a new test [`Keyset`].
pub fn new_test_keyset(
    key_data: KeyData,
    primary_output_prefix_type: tink::proto::OutputPrefixType,
) -> Keyset {
    let primary_key = new_key(
        &key_data,
        tink::proto::KeyStatusType::Enabled,
        42,
        primary_output_prefix_type,
    );
    let raw_key = new_key(
        &key_data,
        tink::proto::KeyStatusType::Enabled,
        43,
        tink::proto::OutputPrefixType::Raw,
    );
    let legacy_key = new_key(
        &key_data,
        tink::proto::KeyStatusType::Enabled,
        44,
        tink::proto::OutputPrefixType::Legacy,
    );
    let tink_key = new_key(
        &key_data,
        tink::proto::KeyStatusType::Enabled,
        45,
        tink::proto::OutputPrefixType::Tink,
    );
    let crunchy_key = new_key(
        &key_data,
        tink::proto::KeyStatusType::Enabled,
        46,
        tink::proto::OutputPrefixType::Crunchy,
    );
    let primary_key_id = primary_key.key_id;
    let keys = vec![primary_key, raw_key, legacy_key, tink_key, crunchy_key];
    new_keyset(primary_key_id, keys)
}

/// Return a dummy key that doesn't contain actual key material.
pub fn new_dummy_key(
    key_id: u32,
    status: tink::proto::KeyStatusType,
    output_prefix_type: tink::proto::OutputPrefixType,
) -> tink::proto::keyset::Key {
    tink::proto::keyset::Key {
        key_data: Some(KeyData::default()),
        status: status as i32,
        key_id,
        output_prefix_type: output_prefix_type as i32,
    }
}

/// Create an [`EcdsaParams`](tink::proto::EcdsaParams) with the specified parameters.
pub fn new_ecdsa_params(
    hash_type: tink::proto::HashType,
    curve: tink::proto::EllipticCurveType,
    encoding: tink::proto::EcdsaSignatureEncoding,
) -> tink::proto::EcdsaParams {
    tink::proto::EcdsaParams {
        hash_type: hash_type as i32,
        curve: curve as i32,
        encoding: encoding as i32,
    }
}

/// Create an [`EcdsaKeyFormat`](tink::proto::EcdsaKeyFormat) with the specified parameters.
pub fn new_ecdsa_key_format(params: tink::proto::EcdsaParams) -> tink::proto::EcdsaKeyFormat {
    tink::proto::EcdsaKeyFormat {
        params: Some(params),
    }
}

/// Create an [`EcdsaPrivateKey`](tink::proto::EcdsaPrivateKey) with the specified paramaters.
pub fn new_ecdsa_private_key(
    version: u32,
    public_key: tink::proto::EcdsaPublicKey,
    key_value: &[u8],
) -> tink::proto::EcdsaPrivateKey {
    tink::proto::EcdsaPrivateKey {
        version,
        public_key: Some(public_key),
        key_value: key_value.to_vec(),
    }
}

/// Create an [`EcdsaPublicKey`](tink::proto::EcdsaPublicKey) with the specified paramaters.
pub fn new_ecdsa_public_key(
    version: u32,
    params: tink::proto::EcdsaParams,
    x: &[u8],
    y: &[u8],
) -> tink::proto::EcdsaPublicKey {
    tink::proto::EcdsaPublicKey {
        version,
        params: Some(params),
        x: x.to_vec(),
        y: y.to_vec(),
    }
}

/// Create an [`EcdsaPrivateKey`](tink::proto::EcdsaPrivateKey) with randomly generated key
/// material.
/* TODO: need ecdsa
pub fn new_random_ecdsa_private_key(
    hash_type: tink::proto::HashType,
    curve: tink::proto::EllipticCurveType,
) -> tink::proto::EcdsaPrivateKey {
    // Prost's implementation of the `Debug` trait for enums gives CamelCase strings.
    let curve_name = format!("{:?}", curve);
    let priv_key = ecdsa::generate_key(tink::subtle::get_curve(curve_name), thread_rng()).unwrap();
    let params = new_ecdsa_params(hash_type, curve, tink::proto::EcdsaSignatureEncoding::Der);
    let public_key = new_ecdsa_public_key(
        ECDSA_VERIFIER_KEY_VERSION,
        params,
        priv_key.X.Bytes(),
        priv_key.Y.Bytes(),
    );
    new_ecdsa_private_key(ECDSA_SIGNER_KEY_VERSION, public_key, priv_key.D.Bytes())
}

/// Create a [`KeyData`] containing an [`EcdsaPrivateKey`](tink::proto::EcdsaPrivateKey) with
/// randomly generated key material.
pub fn new_random_ecdsa_private_key_data(
    hash_type: tink::proto::HashType,
    curve: tink::proto::EllipticCurveType,
) -> KeyData {
    let key = new_random_ecdsa_private_key(hash_type, curve);
    let serialized_key = proto_encode(key);
    KeyData {
        type_url: ECDSA_SIGNER_TYPE_URL.to_string(),
        value: serialized_key,
        key_material_type: tink::proto::key_data::KeyMaterialType::AsymmetricPrivate as i32,
    }
}

/// Create an [`EcdsaPublicKey`](tink::proto::EcdsaPublicKey) with randomly generated key material.
pub fn new_random_ecdsa_public_key(
    hash_type: tink::proto::HashType,
    curve: tink::proto::EllipticCurveType,
) -> tink::proto::EcdsaPublicKey {
    new_random_ecdsa_private_key(hash_type, curve)
        .public_key
        .unwrap()
}
*/

/// Return the string representations of each parameter in the given
/// [`EcdsaParams`](tink::proto::EcdsaParams).
pub fn get_ecdsa_param_names(params: &tink::proto::EcdsaParams) -> (String, String, String) {
    // Prost's implementation of the `Debug` trait for enums gives
    // CamelCase strings (e.g. "Curve25519")
    let hash_name = format!(
        "{:?}",
        tink::proto::HashType::from_i32(params.hash_type).unwrap()
    );
    let curve_name = format!(
        "{:?}",
        tink::proto::EllipticCurveType::from_i32(params.curve).unwrap()
    );
    let encoding_name = format!(
        "{:?}",
        tink::proto::EcdsaSignatureEncoding::from_i32(params.encoding).unwrap()
    );
    (hash_name, curve_name, encoding_name)
}

/// Create an [`Ed25519PrivateKey`](tink::proto::Ed25519PrivateKey) with randomly generated key
/// material.
/* TODO need ed25519
pub fn new_ed25519_private_key() -> tink::proto::Ed25519PrivateKey {
    let (public, private) = ed25519::generate_key(thread_rng()).unwrap();
    let public_proto = tink::proto::Ed25519PublicKey {
        version: ED25519_SIGNER_KEY_VERSION,
        key_value: public,
    };
    tink::proto::Ed25519PrivateKey {
        version: ED25519_SIGNER_KEY_VERSION,
        public_key: Some(public_proto),
        key_value: private.seed(),
    }
}

/// Create a [`KeyData`] containing an [`Ed25519PrivateKey`](tink::proto::Ed25519PrivateKey) with randomly generated key material.
pub fn new_ed25519_private_key_data() -> KeyData {
    let key = new_ed25519_private_key();
    let serialized_key = proto_encode(key);
    KeyData {
        type_url: ED25519_SIGNER_TYPE_URL.to_string(),
        value: serialized_key,
        key_material_type: tink::proto::key_data::KeyMaterialType::AsymmetricPrivate as i32,
    }
}

/// Create an [`Ed25519PublicKey`](tink::proto::Ed25519PublicKey) with randomly generated key material.
pub fn new_ed25519_public_key() -> tink::proto::Ed25519PublicKey {
    new_ed25519_private_key().public_key.unwrap()
}
*/

/// Create a randomly generated [`AesGcmKey`](tink::proto::AesGcmKey).
pub fn new_aes_gcm_key(key_version: u32, key_size: u32) -> tink::proto::AesGcmKey {
    let key_value = get_random_bytes(key_size.try_into().unwrap());
    tink::proto::AesGcmKey {
        version: key_version,
        key_value,
    }
}

/// Create a [`KeyData`] containing a randomly generated
/// [`AesGcmKey`](tink::proto::AesGcmKey).
pub fn new_aes_gcm_key_data(key_size: u32) -> KeyData {
    let key = new_aes_gcm_key(AES_GCM_KEY_VERSION, key_size);
    let serialized_key = proto_encode(&key);
    new_key_data(
        AES_GCM_TYPE_URL,
        &serialized_key,
        tink::proto::key_data::KeyMaterialType::Symmetric,
    )
}

/// Create an [`AesGcmKey`](tink::proto::AesGcmKey) with randomly generated key material.
pub fn new_serialized_aes_gcm_key(key_size: u32) -> Vec<u8> {
    let key = new_aes_gcm_key(AES_GCM_KEY_VERSION, key_size);
    proto_encode(&key)
}

/// Return a new [`AesGcmKeyFormat`](tink::proto::AesGcmKeyFormat).
pub fn new_aes_gcm_key_format(key_size: u32) -> tink::proto::AesGcmKeyFormat {
    tink::proto::AesGcmKeyFormat {
        key_size,
        version: AES_GCM_KEY_VERSION,
    }
}

/// Create a randomly generated [`AesGcmHkdfStreamingKey`](tink::proto::AesGcmHkdfStreamingKey).
pub fn new_aes_gcm_hkdf_key(
    key_version: u32,
    key_size: u32,
    derived_key_size: u32,
    hkdf_hash_type: tink::proto::HashType,
    ciphertext_segment_size: u32,
) -> tink::proto::AesGcmHkdfStreamingKey {
    let key_value = get_random_bytes(key_size.try_into().unwrap());
    tink::proto::AesGcmHkdfStreamingKey {
        version: key_version,
        key_value,
        params: Some(tink::proto::AesGcmHkdfStreamingParams {
            ciphertext_segment_size,
            derived_key_size,
            hkdf_hash_type: hkdf_hash_type as i32,
        }),
    }
}

/// Create a [`KeyData`] containing a randomly generated
/// [`AesGcmHkdfStreamingKey`](tink::proto::AesGcmHkdfStreamingKey).
pub fn new_aes_gcm_hkdf_key_data(
    key_size: u32,
    derived_key_size: u32,
    hkdf_hash_type: tink::proto::HashType,
    ciphertext_segment_size: u32,
) -> KeyData {
    let key = new_aes_gcm_hkdf_key(
        AES_GCM_HKDF_KEY_VERSION,
        key_size,
        derived_key_size,
        hkdf_hash_type,
        ciphertext_segment_size,
    );
    let serialized_key = proto_encode(&key);
    new_key_data(
        AES_GCM_HKDF_TYPE_URL,
        &serialized_key,
        tink::proto::key_data::KeyMaterialType::Symmetric,
    )
}

/// Return a new [`AesGcmHkdfStreamingKeyFormat`](tink::proto::AesGcmHkdfStreamingKeyFormat).
pub fn new_aes_gcm_hkdf_key_format(
    key_size: u32,
    derived_key_size: u32,
    hkdf_hash_type: tink::proto::HashType,
    ciphertext_segment_size: u32,
) -> tink::proto::AesGcmHkdfStreamingKeyFormat {
    tink::proto::AesGcmHkdfStreamingKeyFormat {
        version: AES_GCM_HKDF_KEY_VERSION,
        key_size,
        params: Some(tink::proto::AesGcmHkdfStreamingParams {
            ciphertext_segment_size,
            derived_key_size,
            hkdf_hash_type: hkdf_hash_type as i32,
        }),
    }
}

/// Create a randomly generated [`AesCtrHmacStreamingKey`](tink::proto::AesCtrHmacStreamingKey).
pub fn new_aes_ctr_hmac_key(
    key_version: u32,
    key_size: u32,
    hkdf_hash_type: tink::proto::HashType,
    derived_key_size: u32,
    hash_type: tink::proto::HashType,
    tag_size: u32,
    ciphertext_segment_size: u32,
) -> tink::proto::AesCtrHmacStreamingKey {
    let key_value = get_random_bytes(key_size.try_into().unwrap());
    tink::proto::AesCtrHmacStreamingKey {
        version: key_version,
        key_value,
        params: Some(tink::proto::AesCtrHmacStreamingParams {
            ciphertext_segment_size,
            derived_key_size,
            hkdf_hash_type: hkdf_hash_type as i32,
            hmac_params: Some(tink::proto::HmacParams {
                hash: hash_type as i32,
                tag_size,
            }),
        }),
    }
}

/// Create a [`KeyData`] containing a randomly generated
/// [`AesCtrHmacStreamingKey`](tink::proto::AesCtrHmacStreamingKey).
pub fn new_aes_ctr_hmac_key_data(
    key_size: u32,
    hkdf_hash_type: tink::proto::HashType,
    derived_key_size: u32,
    hash_type: tink::proto::HashType,
    tag_size: u32,
    ciphertext_segment_size: u32,
) -> KeyData {
    let key = new_aes_ctr_hmac_key(
        AES_CTR_HMAC_KEY_VERSION,
        key_size,
        hkdf_hash_type,
        derived_key_size,
        hash_type,
        tag_size,
        ciphertext_segment_size,
    );
    let serialized_key = proto_encode(&key);
    new_key_data(
        AES_CTR_HMAC_TYPE_URL,
        &serialized_key,
        tink::proto::key_data::KeyMaterialType::Symmetric,
    )
}

/// Return a new [`AesCtrHmacStreamingKeyFormat`](tink::proto::AesCtrHmacStreamingKeyFormat).
pub fn new_aes_ctr_hmac_key_format(
    key_size: u32,
    hkdf_hash_type: tink::proto::HashType,
    derived_key_size: u32,
    hash_type: tink::proto::HashType,
    tag_size: u32,
    ciphertext_segment_size: u32,
) -> tink::proto::AesCtrHmacStreamingKeyFormat {
    tink::proto::AesCtrHmacStreamingKeyFormat {
        key_size,
        params: Some(tink::proto::AesCtrHmacStreamingParams {
            ciphertext_segment_size,
            derived_key_size,
            hkdf_hash_type: hkdf_hash_type as i32,
            hmac_params: Some(tink::proto::HmacParams {
                hash: hash_type as i32,
                tag_size,
            }),
        }),
    }
}

/// Return a new [`HmacParams`](tink::proto::HmacParams).
pub fn new_hmac_params(hash_type: tink::proto::HashType, tag_size: u32) -> tink::proto::HmacParams {
    tink::proto::HmacParams {
        hash: hash_type as i32,
        tag_size,
    }
}

/// Create a new [`HmacKey`](tink::proto::HmacKey) with the specified parameters.
pub fn new_hmac_key(hash_type: tink::proto::HashType, tag_size: u32) -> tink::proto::HmacKey {
    let params = new_hmac_params(hash_type, tag_size);
    let key_value = get_random_bytes(20);
    tink::proto::HmacKey {
        version: HMAC_KEY_VERSION,
        params: Some(params),
        key_value,
    }
}

/// Create a new [`HmacKeyFormat`](tink::proto::HmacKeyFormat) with the specified parameters.
pub fn new_hmac_key_format(
    hash_type: tink::proto::HashType,
    tag_size: u32,
) -> tink::proto::HmacKeyFormat {
    let params = new_hmac_params(hash_type, tag_size);
    let key_size = 20u32;
    tink::proto::HmacKeyFormat {
        params: Some(params),
        key_size,
        version: HMAC_KEY_VERSION,
    }
}

/// Return a new [`AesCmacParams`](tink::proto::AesCmacParams).
pub fn new_aes_cmac_params(tag_size: u32) -> tink::proto::AesCmacParams {
    tink::proto::AesCmacParams { tag_size }
}

/// Create a new [`AesCmacKey`](tink::proto::AesCmacKey) with the specified parameters.
pub fn new_aes_cmac_key(tag_size: u32) -> tink::proto::AesCmacKey {
    let params = new_aes_cmac_params(tag_size);
    let key_value = get_random_bytes(32);
    tink::proto::AesCmacKey {
        version: AES_CMAC_KEY_VERSION,
        params: Some(params),
        key_value,
    }
}

/// Create a new [`AesCmacKeyFormat`](tink::proto::AesCmacKeyFormat) with the specified parameters.
pub fn new_aes_cmac_key_format(tag_size: u32) -> tink::proto::AesCmacKeyFormat {
    let params = new_aes_cmac_params(tag_size);
    let key_size = 32u32;
    tink::proto::AesCmacKeyFormat {
        params: Some(params),
        key_size,
    }
}

/// Return a new `tink::keyset::Manager` that contains a [`HmacKey`](tink::proto::HmacKey).
/* TODO need hmac_sha256
pub fn new_hmac_keyset_manager() -> tink::keyset::Manager {
    let ksm = tink::keyset::Manager::new();
    let kt = mac::hmac_sha256_tag128_key_template();
    ksm.rotate(kt).expect("cannot rotate keyset manager");
    ksm
}
*/

/// Return a new [`KeyData`] that contains a [`HmacKey`](tink::proto::HmacKey).
pub fn new_hmac_key_data(hash_type: tink::proto::HashType, tag_size: u32) -> KeyData {
    let key = new_hmac_key(hash_type, tag_size);
    let serialized_key = proto_encode(&key);
    KeyData {
        type_url: HMAC_TYPE_URL.to_string(),
        value: serialized_key,
        key_material_type: tink::proto::key_data::KeyMaterialType::Symmetric as i32,
    }
}

/// Return a new [`HmacPrfParams`](tink::proto::HmacPrfParams).
pub fn new_hmac_prf_params(hash_type: tink::proto::HashType) -> tink::proto::HmacPrfParams {
    tink::proto::HmacPrfParams {
        hash: hash_type as i32,
    }
}

/// Create a new [`HmacPrfKey`](tink::proto::HmacPrfKey) with the specified parameters.
pub fn new_hmac_prf_key(hash_type: tink::proto::HashType) -> tink::proto::HmacPrfKey {
    let params = new_hmac_prf_params(hash_type);
    let key_value = get_random_bytes(32);
    tink::proto::HmacPrfKey {
        version: HMAC_PRF_KEY_VERSION,
        params: Some(params),
        key_value,
    }
}

/// Create a new [`HmacPrfKeyFormat`](tink::proto::HmacPrfKeyFormat) with the specified parameters.
pub fn new_hmac_prf_key_format(hash_type: tink::proto::HashType) -> tink::proto::HmacPrfKeyFormat {
    let params = new_hmac_prf_params(hash_type);
    let key_size = 32u32;
    tink::proto::HmacPrfKeyFormat {
        params: Some(params),
        key_size,
        version: HMAC_PRF_KEY_VERSION,
    }
}

/// Return a new [`HkdfPrfParams`](tink::proto::HkdfPrfParams).
pub fn new_hkdf_prf_params(
    hash_type: tink::proto::HashType,
    salt: &[u8],
) -> tink::proto::HkdfPrfParams {
    tink::proto::HkdfPrfParams {
        hash: hash_type as i32,
        salt: salt.to_vec(),
    }
}

/// Create a new [`HkdfPrfKey`](tink::proto::HkdfPrfKey) with the specified parameters.
pub fn new_hkdf_prf_key(hash_type: tink::proto::HashType, salt: &[u8]) -> tink::proto::HkdfPrfKey {
    let params = new_hkdf_prf_params(hash_type, salt);
    let key_value = get_random_bytes(32);
    tink::proto::HkdfPrfKey {
        version: HKDF_PRF_KEY_VERSION,
        params: Some(params),
        key_value,
    }
}

/// Create a new [`HkdfPrfKeyFormat`](tink::proto::HkdfPrfKeyFormat) with the specified parameters.
pub fn new_hkdf_prf_key_format(
    hash_type: tink::proto::HashType,
    salt: &[u8],
) -> tink::proto::HkdfPrfKeyFormat {
    let params = new_hkdf_prf_params(hash_type, salt);
    let key_size = 32u32;
    tink::proto::HkdfPrfKeyFormat {
        params: Some(params),
        key_size,
        version: HKDF_PRF_KEY_VERSION,
    }
}

/// Create a new [`AesCmacPrfKey`](tink::proto::AesCmacPrfKey) with the specified parameters.
pub fn new_aes_cmac_prf_key() -> tink::proto::AesCmacPrfKey {
    let key_value = get_random_bytes(32);
    tink::proto::AesCmacPrfKey {
        version: AES_CMAC_PRF_KEY_VERSION,
        key_value,
    }
}

/// Create a new [`AesCmacPrfKeyFormat`](tink::proto::AesCmacPrfKeyFormat) with the specified
/// parameters.
pub fn new_aes_cmac_prf_key_format() -> tink::proto::AesCmacPrfKeyFormat {
    let key_size = 32u32;
    tink::proto::AesCmacPrfKeyFormat {
        version: AES_CMAC_PRF_KEY_VERSION,
        key_size,
    }
}

/// Create a new [`KeyData`] with the specified parameters.
pub fn new_key_data(
    type_url: &str,
    value: &[u8],
    material_type: tink::proto::key_data::KeyMaterialType,
) -> KeyData {
    KeyData {
        type_url: type_url.to_string(),
        value: value.to_vec(),
        key_material_type: material_type as i32,
    }
}

/// Create a new [`Key`](tink::proto::keyset::Key) with the specified parameters.
pub fn new_key(
    key_data: &KeyData,
    status: tink::proto::KeyStatusType,
    key_id: u32,
    prefix_type: tink::proto::OutputPrefixType,
) -> tink::proto::keyset::Key {
    tink::proto::keyset::Key {
        key_data: Some(key_data.clone()),
        status: status as i32,
        key_id,
        output_prefix_type: prefix_type as i32,
    }
}

/// Create a new [`Keyset`] with the specified parameters.
pub fn new_keyset(primary_key_id: u32, keys: Vec<tink::proto::keyset::Key>) -> Keyset {
    Keyset {
        primary_key_id,
        key: keys,
    }
}

/// Create a new [`EncryptedKeyset`](tink::proto::EncryptedKeyset) with a specified parameters.
pub fn new_encrypted_keyset(
    encrypted_keyset: &[u8],
    info: tink::proto::KeysetInfo,
) -> tink::proto::EncryptedKeyset {
    tink::proto::EncryptedKeyset {
        encrypted_keyset: encrypted_keyset.to_vec(),
        keyset_info: Some(info),
    }
}

/// Generate different byte mutations for a given byte array.
pub fn generate_mutations(src: &[u8]) -> Vec<Vec<u8>> {
    let mut all = Vec::new();

    // Flip bits
    for i in 0..src.len() {
        for j in 0..8u8 {
            let mut n = src.to_vec();
            n[i] ^= 1 << j;
            all.push(n);
        }
    }

    // truncate bytes
    for i in 0..src.len() {
        all.push(src[i..].to_vec());
    }

    // append extra byte
    let mut m = src.to_vec();
    m.push(0);
    all.push(m);
    all
}

/// Use a z test on the given byte string, expecting all bits to be uniformly set with probability
/// 1/2. Returns non ok status if the z test fails by more than 10 standard deviations.
///
/// With less statistics jargon: This counts the number of bits set and expects the number to be
/// roughly half of the length of the string. The law of large numbers suggests that we can assume
/// that the longer the string is, the more accurate that estimate becomes for a random string. This
/// test is useful to detect things like strings that are entirely zero.
///
/// Note: By itself, this is a very weak test for randomness.
pub fn z_test_uniform_string(bytes: &[u8]) -> Result<(), tink::TinkError> {
    let expected = (bytes.len() as f64) * 8.0 / 2.0;
    let stddev = ((bytes.len() as f64) * 8.0 / 4.0).sqrt();
    let mut num_set_bits: i64 = 0;
    for b in bytes {
        // Counting the number of bits set in byte:
        let mut b = *b;
        while b != 0 {
            num_set_bits += 1;
            b = b & (b - 1);
        }
    }
    // Check that the number of bits is within 10 stddevs.
    if ((num_set_bits as f64) - expected).abs() < 10.0 * stddev {
        Ok(())
    } else {
        Err(format!(
                "Z test for uniformly distributed variable out of bounds; Actual number of set bits was {} expected was {}, 10 * standard deviation is 10 * {} = {}",
            num_set_bits, expected, stddev, 10.0*stddev).into())
    }
}

fn rotate(bytes: &[u8]) -> Vec<u8> {
    let mut result = vec![0u8; bytes.len()];
    for i in 0..bytes.len() {
        let prev = if i == 0 { bytes.len() } else { i };
        result[i] = (bytes[i] >> 1) | (bytes[prev - 1] << 7);
    }
    result
}

/// Test that the cross-correlation of two byte strings of equal length points to independent and
/// uniformly distributed strings. Returns non `Ok` status if the z test fails by more than 10
/// standard deviations.
///
/// With less statistics jargon: This xors two strings and then performs the z_test_uniform_string
/// on the result. If the two strings are independent and uniformly distributed, the xor'ed string
/// is as well. A cross correlation test will find whether two strings overlap more or less than it
/// would be expected.
///
/// Note: Having a correlation of zero is only a necessary but not sufficient condition for
/// independence.
pub fn z_test_crosscorrelation_uniform_strings(
    bytes1: &[u8],
    bytes2: &[u8],
) -> Result<(), TinkError> {
    if bytes1.len() != bytes2.len() {
        return Err("Strings are not of equal length".into());
    }
    let mut crossed = vec![0u8; bytes1.len()];
    for i in 0..bytes1.len() {
        crossed[i] = bytes1[i] ^ bytes2[i]
    }
    z_test_uniform_string(&crossed)
}

/// Test whether the autocorrelation of a string
/// points to the bits being independent and uniformly distributed.
/// Rotates the string in a cyclic fashion. Returns non ok status if the z test
/// fails by more than 10 standard deviations.
///
/// With less statistics jargon: This rotates the string bit by bit and performs
/// z_test_crosscorrelation_uniform_strings on each of the rotated strings and the
/// original. This will find self similarity of the input string, especially
/// periodic self similarity. For example, it is a decent test to find English
/// text (needs about 180 characters with the current settings).
///
/// Note: Having a correlation of zero is only a necessary but not sufficient
/// condition for independence.
pub fn z_test_autocorrelation_uniform_string(bytes: &[u8]) -> Result<(), TinkError> {
    let mut rotated = bytes.to_vec();
    let mut violations = Vec::new();
    for i in 1..(bytes.len() * 8) {
        rotated = rotate(&rotated);
        if z_test_crosscorrelation_uniform_strings(bytes, &rotated).is_err() {
            violations.push(i.to_string());
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(TinkError::new(&format!(
            "Autocorrelation exceeded 10 standard deviation at {} indices: {}",
            violations.len(),
            violations.join(", ")
        )))
    }
}

/// Return a [`EciesAeadHkdfPublicKey`](tink::proto::EciesAeadHkdfPublicKey) with specified
/// parameters.
pub fn ecies_aead_hkdf_public_key(
    c: tink::proto::EllipticCurveType,
    ht: tink::proto::HashType,
    ptfmt: tink::proto::EcPointFormat,
    dek_t: tink::proto::KeyTemplate,
    x: &[u8],
    y: &[u8],
    salt: &[u8],
) -> tink::proto::EciesAeadHkdfPublicKey {
    tink::proto::EciesAeadHkdfPublicKey {
        version: 0,
        params: Some(tink::proto::EciesAeadHkdfParams {
            kem_params: Some(tink::proto::EciesHkdfKemParams {
                curve_type: c as i32,
                hkdf_hash_type: ht as i32,
                hkdf_salt: salt.to_vec(),
            }),
            dem_params: Some(tink::proto::EciesAeadDemParams {
                aead_dem: Some(dek_t),
            }),
            ec_point_format: ptfmt as i32,
        }),
        x: x.to_vec(),
        y: y.to_vec(),
    }
}

/// Return an [`EciesAeadHkdfPrivateKey`](tink::proto::EciesAeadHkdfPrivateKey) with specified
/// parameters
pub fn ecies_aead_hkdf_private_key(
    p: tink::proto::EciesAeadHkdfPublicKey,
    d: &[u8],
) -> tink::proto::EciesAeadHkdfPrivateKey {
    tink::proto::EciesAeadHkdfPrivateKey {
        version: 0,
        public_key: Some(p),
        key_value: d.to_vec(),
    }
}

/// Generate a new EC key pair and returns the private key proto.
pub fn generate_ecies_aead_hkdf_private_key(
    c: tink::proto::EllipticCurveType,
    ht: tink::proto::HashType,
    ptfmt: tink::proto::EcPointFormat,
    dek_t: tink::proto::KeyTemplate,
    salt: &[u8],
) -> Result<tink::proto::EciesAeadHkdfPrivateKey, TinkError> {
    // TODO: implementation via ECC library
    /*
    let curve = subtlehybrid.get_curve(format!("{:?}", c))?;
    let pvt = subtlehybrid.generate_ecdh_key_pair(curve)?;
    let pubKey = ecies_aead_hkdf_public_key(
        c,
        ht,
        ptfmt,
        dek_t,
        pvt.public_key.point.x.bytes(),
        pvt.public_key.point.y.bytes(),
        salt,
    );
    Ok(ecies_aead_hkdf_private_key(pubKey, pvt.d.Bytes()))
     */
    Err(format!(
        "unimplemented for {:?} {:?} {:?} {:?} {:?}",
        c, ht, ptfmt, dek_t, salt
    )
    .into())
}

/// Convert a protocol buffer message to its serialized form.
pub fn proto_encode<T>(msg: &T) -> Vec<u8>
where
    T: prost::Message,
{
    let mut data = Vec::new();
    msg.encode(&mut data)
        .expect("failed to encode proto message");
    data
}