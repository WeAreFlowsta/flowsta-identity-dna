//! Flowsta Identity DNA v1.5 on a real Holochain 0.7.1 conductor.
//!
//! Loads the packed bundle from `../workdir/flowsta_identity_v1_5.dna` (the
//! bytes that ship). Covers the users zome round trip and every "valid when"
//! row of the devices zome (planning/DEVICE_CERTIFICATES_0_7.md § 4) plus the
//! attack rows that validation decides: forged signatures, a second
//! enrollment key without the seven-day delay, a cancellation after effect,
//! a removal by an enrollment key not yet in force, a device removing itself.

use devices_coordinator::{
    CancelEnrollmentKeyInput, EnrollmentKeyInForce, InForceInput, RegisterEnrollmentKeyInput, RemoveDeviceInput,
};
use devices_integrity::{
    certificate_statement, device_hash_of, enrollment_cancel_statement, enrollment_statement, removal_statement,
    DeviceCertificate, DeviceRemoved, EnrollmentKey, RemovalSigner, SEVEN_DAYS_MS,
};
use ed25519_dalek::{Signer, SigningKey};
use holochain::prelude::*;
use holochain::sweettest::*;
use std::path::Path;
use holochain_serialized_bytes::prelude::SerializedBytes as SerializedBytesDerive;

/// The users zome's profile, as its input and entry serialize (field names
/// are the wire format). Declared here rather than imported: two integrity
/// crates in one test binary export the same `validate` symbol.
#[derive(serde::Serialize, serde::Deserialize, SerializedBytesDerive, Clone, Debug)]
struct UserProfile {
    did: String,
    created_at: i64,
    updated_at: i64,
}

async fn dna() -> DnaFile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../workdir/flowsta_identity_v1_5.dna");
    SweetDnaFile::from_bundle(&path)
        .await
        .expect("v1.5/workdir/flowsta_identity_v1_5.dna loads (run build.sh first)")
}

/// An app-side key (identity, enrollment or device): the Vault holds these;
/// the conductor's agent only authors the records.
struct Key {
    signing: SigningKey,
}
impl Key {
    fn new() -> Self {
        Key { signing: SigningKey::generate(&mut rand::rngs::OsRng) }
    }
    fn public(&self) -> Vec<u8> {
        self.signing.verifying_key().to_bytes().to_vec()
    }
    fn agent(&self) -> AgentPubKey {
        AgentPubKey::from_raw_32(self.public())
    }
    fn sign(&self, statement: &str) -> Vec<u8> {
        self.signing.sign(statement.as_bytes()).to_bytes().to_vec()
    }
}

fn decode<T: TryFrom<SerializedBytes, Error = SerializedBytesError>>(record: &Record) -> T {
    record.entry().to_app_option::<T>().ok().flatten().expect("record carries the expected app entry")
}

fn certificate(identity: &Key, device: &Key, install_id: &str, issued_at: i64) -> DeviceCertificate {
    let mut c = DeviceCertificate {
        identity: identity.agent(),
        device_key: device.public(),
        install_id: install_id.to_string(),
        capabilities: "approve,login,read,sign".to_string(),
        issued_at,
        identity_signature: vec![],
        enrollment_signature: None,
    };
    c.identity_signature = identity.sign(&certificate_statement(&c));
    c
}

/// The users zome, unchanged from v1.4, on 0.7.1.
#[tokio::test(flavor = "multi_thread")]
async fn users_round_trip() {
    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("identity", [&dna().await]).await.unwrap();
    let zome = app.cells()[0].zome("users");
    let profile = UserProfile { did: "did:flowsta:uTEST".into(), created_at: 1, updated_at: 1 };
    let _: Record = conductor.call(&zome, "register_user", profile.clone()).await;
    let mine: Option<Record> = conductor.call(&zome, "get_my_profile", ()).await;
    let p: UserProfile = decode(&mine.expect("profile"));
    assert_eq!(p.did, "did:flowsta:uTEST");
    let _: Record = conductor.call(&zome, "update_profile", UserProfile { updated_at: 2, ..profile }).await;
    let by_agent: Option<Record> = conductor.call(&zome, "get_profile", app.agent().clone()).await;
    let p: UserProfile = decode(&by_agent.expect("profile by agent"));
    assert_eq!(p.updated_at, 2, "the update chain is followed");
    let exported: UserProfile = conductor.call(&zome, "export_all_data", ()).await;
    assert_eq!(exported.updated_at, 2);
}

/// Every row of the devices zome's validation, through the coordinator.
#[tokio::test(flavor = "multi_thread")]
async fn device_records() {
    let mut conductor = SweetConductor::standard().await;
    let app = conductor.setup_app("identity", [&dna().await]).await.unwrap();
    let zome = app.cells()[0].zome("devices");
    let identity = Key::new();
    let e1 = Key::new();
    let e2 = Key::new();
    let t0: i64 = 1_790_000_000_000;

    // ── EnrollmentKey ──
    // A statement signed by another key is refused.
    let r: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "register_enrollment_key", RegisterEnrollmentKeyInput {
            identity: identity.agent(), enrollment_key: e1.public(), created_at: t0, previous: None,
            identity_signature: e1.sign(&enrollment_statement(&identity.agent(), &e1.public(), t0)),
        })
        .await;
    assert!(r.is_err(), "an enrollment record must be signed by the identity");

    // The first record takes effect at once.
    let first: ActionHash = conductor
        .call(&zome, "register_enrollment_key", RegisterEnrollmentKeyInput {
            identity: identity.agent(), enrollment_key: e1.public(), created_at: t0, previous: None,
            identity_signature: identity.sign(&enrollment_statement(&identity.agent(), &e1.public(), t0)),
        })
        .await;
    let keys: Vec<Record> = conductor.call(&zome, "get_enrollment_keys", identity.agent()).await;
    assert_eq!(keys.len(), 1);
    let k: EnrollmentKey = decode(&keys[0]);
    assert_eq!(k.effective_at, t0);
    let in_force: Option<EnrollmentKeyInForce> = conductor
        .call(&zome, "enrollment_key_in_force", InForceInput { identity: identity.agent(), at: t0 })
        .await;
    assert_eq!(in_force.expect("e1 in force").enrollment_key, e1.public());

    // A later record must name the first and takes effect seven days on.
    let t1 = t0 + 60_000;
    let second: ActionHash = conductor
        .call(&zome, "register_enrollment_key", RegisterEnrollmentKeyInput {
            identity: identity.agent(), enrollment_key: e2.public(), created_at: t1, previous: Some(first.clone()),
            identity_signature: identity.sign(&enrollment_statement(&identity.agent(), &e2.public(), t1)),
        })
        .await;
    let keys: Vec<Record> = conductor.call(&zome, "get_enrollment_keys", identity.agent()).await;
    let k2: EnrollmentKey = keys.iter().map(decode::<EnrollmentKey>).find(|k| k.enrollment_key == e2.public()).unwrap();
    assert_eq!(k2.effective_at, t1 + SEVEN_DAYS_MS);
    // Not in force yet at t1 + 1 day; in force at t1 + 8 days.
    let f: Option<EnrollmentKeyInForce> = conductor
        .call(&zome, "enrollment_key_in_force", InForceInput { identity: identity.agent(), at: t1 + 86_400_000 })
        .await;
    assert_eq!(f.unwrap().enrollment_key, e1.public());
    let f: Option<EnrollmentKeyInForce> = conductor
        .call(&zome, "enrollment_key_in_force", InForceInput { identity: identity.agent(), at: t1 + SEVEN_DAYS_MS + 86_400_000 })
        .await;
    assert_eq!(f.unwrap().enrollment_key, e2.public());

    // A later record created before its predecessor is refused.
    let r: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "register_enrollment_key", RegisterEnrollmentKeyInput {
            identity: identity.agent(), enrollment_key: Key::new().public(), created_at: t0 - 1, previous: Some(first.clone()),
            identity_signature: identity.sign(&enrollment_statement(&identity.agent(), &e2.public(), t0 - 1)),
        })
        .await;
    assert!(r.is_err());

    // ── EnrollmentCancelled ──
    // Cancelling the second key before it takes effect: valid; its key is no longer in force later.
    let r: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "cancel_enrollment_key", CancelEnrollmentKeyInput {
            identity: identity.agent(), enrollment_record: second.clone(), enrollment_key: e2.public(), at: t1 + SEVEN_DAYS_MS + 1,
            identity_signature: identity.sign(&enrollment_cancel_statement(&identity.agent(), &e2.public(), t1 + SEVEN_DAYS_MS + 1)),
        })
        .await;
    assert!(r.is_err(), "a cancellation after effect is refused");
    let _: ActionHash = conductor
        .call(&zome, "cancel_enrollment_key", CancelEnrollmentKeyInput {
            identity: identity.agent(), enrollment_record: second.clone(), enrollment_key: e2.public(), at: t1 + 1000,
            identity_signature: identity.sign(&enrollment_cancel_statement(&identity.agent(), &e2.public(), t1 + 1000)),
        })
        .await;
    let f: Option<EnrollmentKeyInForce> = conductor
        .call(&zome, "enrollment_key_in_force", InForceInput { identity: identity.agent(), at: t1 + SEVEN_DAYS_MS + 86_400_000 })
        .await;
    assert_eq!(f.unwrap().enrollment_key, e1.public(), "the cancelled key never takes effect");

    // ── DeviceRemoved ──
    let stolen = Key::new();
    let other = Key::new();
    let t2 = t1 + 2 * 86_400_000;
    // By the identity: valid; found by the device's key.
    let by_identity: ActionHash = conductor
        .call(&zome, "remove_device", RemoveDeviceInput {
            identity: identity.agent(), device_key: stolen.public(), at: t2, signer: RemovalSigner::Identity,
            signature: identity.sign(&removal_statement(&identity.agent(), &device_hash_of(&stolen.public()), t2)),
        })
        .await;
    let found: Option<Record> = conductor.call(&zome, "get_removal_for_device", stolen.public()).await;
    let removed: DeviceRemoved = decode(&found.expect("the removal is found by the device key"));
    assert_eq!(removed.device_hash, device_hash_of(&stolen.public()));
    assert_eq!(removed.at, t2);
    let none: Option<Record> = conductor.call(&zome, "get_removal_for_device", other.public()).await;
    assert!(none.is_none());
    let all: Vec<Record> = conductor.call(&zome, "get_removals", identity.agent()).await;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].action_address(), &by_identity);

    // A forged identity signature is refused.
    let r: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "remove_device", RemoveDeviceInput {
            identity: identity.agent(), device_key: other.public(), at: t2, signer: RemovalSigner::Identity,
            signature: other.sign(&removal_statement(&identity.agent(), &device_hash_of(&other.public()), t2)),
        })
        .await;
    assert!(r.is_err());

    // By the enrollment key e1 (in force since t0): valid.
    let dev2 = Key::new();
    let _: ActionHash = conductor
        .call(&zome, "remove_device", RemoveDeviceInput {
            identity: identity.agent(), device_key: dev2.public(), at: t2, signer: RemovalSigner::Enrollment { enrollment_record: first.clone() },
            signature: e1.sign(&removal_statement(&identity.agent(), &device_hash_of(&dev2.public()), t2)),
        })
        .await;
    // By the enrollment key e2 at a time before its effect: refused.
    let dev3 = Key::new();
    let r: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "remove_device", RemoveDeviceInput {
            identity: identity.agent(), device_key: dev3.public(), at: t2, signer: RemovalSigner::Enrollment { enrollment_record: second.clone() },
            signature: e2.sign(&removal_statement(&identity.agent(), &device_hash_of(&dev3.public()), t2)),
        })
        .await;
    assert!(r.is_err(), "an enrollment key removes only once in force");

    // By a device with its certificate: valid; a forged certificate refused; removing itself refused.
    let remover = Key::new();
    let cert = certificate(&identity, &remover, "install-1", t0);
    let _: ActionHash = conductor
        .call(&zome, "remove_device", RemoveDeviceInput {
            identity: identity.agent(), device_key: dev3.public(), at: t2, signer: RemovalSigner::Device { certificate: cert.clone() },
            signature: remover.sign(&removal_statement(&identity.agent(), &device_hash_of(&dev3.public()), t2)),
        })
        .await;
    let mut forged = cert.clone();
    forged.identity_signature = remover.sign(&certificate_statement(&forged));
    let dev4 = Key::new();
    let r: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "remove_device", RemoveDeviceInput {
            identity: identity.agent(), device_key: dev4.public(), at: t2, signer: RemovalSigner::Device { certificate: forged },
            signature: remover.sign(&removal_statement(&identity.agent(), &device_hash_of(&dev4.public()), t2)),
        })
        .await;
    assert!(r.is_err(), "a certificate needs the identity's signature");
    let r: Result<ActionHash, _> = conductor
        .call_fallible(&zome, "remove_device", RemoveDeviceInput {
            identity: identity.agent(), device_key: remover.public(), at: t2, signer: RemovalSigner::Device { certificate: cert.clone() },
            signature: remover.sign(&removal_statement(&identity.agent(), &device_hash_of(&remover.public()), t2)),
        })
        .await;
    assert!(r.is_err(), "a device cannot remove itself");

    let all: Vec<Record> = conductor.call(&zome, "get_removals", identity.agent()).await;
    assert_eq!(all.len(), 3, "identity, enrollment-key and device removals");
}

/// Records written on one conductor are found from another by the device's key.
#[tokio::test(flavor = "multi_thread")]
async fn removal_reaches_a_verifier_on_another_conductor() {
    let dna = dna().await;
    let mut conductors = SweetConductorBatch::standard(2).await;
    let apps = conductors.setup_app("identity", [&dna]).await.unwrap();
    conductors.exchange_peer_info().await;
    let ((alice,), (bob,)) = apps.into_tuples();
    let a = conductors.get(0).unwrap();
    let b = conductors.get(1).unwrap();
    let identity = Key::new();
    let stolen = Key::new();
    let t = 1_790_000_000_000;
    let _: ActionHash = a
        .call(&alice.zome("devices"), "remove_device", RemoveDeviceInput {
            identity: identity.agent(), device_key: stolen.public(), at: t, signer: RemovalSigner::Identity,
            signature: identity.sign(&removal_statement(&identity.agent(), &device_hash_of(&stolen.public()), t)),
        })
        .await;
    await_consistency([&alice, &bob]).await.expect("the removal reaches bob");
    let found: Option<Record> = b.call(&bob.zome("devices"), "get_removal_for_device", stolen.public()).await;
    assert!(found.is_some(), "a verifier on another conductor finds the tombstone");
}
