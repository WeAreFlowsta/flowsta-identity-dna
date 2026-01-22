# Flowsta Identity DNA

**Censorship-resistant identity storage for distributed authentication**

[![Status](https://img.shields.io/badge/status-production-brightgreen.svg)](https://flowsta.com)
[![Holochain](https://img.shields.io/badge/holochain-0.6.0-blue.svg)](https://holochain.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![DNA Version](https://img.shields.io/badge/DNA-v1.2-orange.svg)](#version-history)

> **🎉 Production Status**: This DNA is currently running in production, powering [Flowsta Auth](https://flowsta.com) - a zero-knowledge authentication system with censorship-resistant identity.

---

## 🔍 What is This?

The **Flowsta Identity DNA** is a Holochain Distributed Hash Table (DHT) that stores **public profile data** in a censorship-resistant manner. Once published, user identities cannot be deleted by site owners, government actors, or even Flowsta itself.

**Part of Flowsta's Dual-DNA Architecture**: This DNA works alongside the [Flowsta Private DNA](https://github.com/WeAreFlowsta/flowsta-private-dna) to provide both censorship resistance AND privacy.

---

## 🏗️ Architecture Overview

### What's Stored on This Public DHT:

| Data | Why Public? | Size Limit |
|------|-------------|------------|
| **W3C DID** | Decentralized Identifier (designed to be public) | 64 chars |
| **Profile Picture** | User's chosen avatar or identicon | < 200KB |
| **Has Custom Picture** | Boolean flag | 1 bit |

### What's NOT Stored Here (Privacy Fix in v1.2):

| Removed in v1.2 | Why Removed | Where It Went |
|-----------------|-------------|---------------|
| ❌ **Email Hash** | Vulnerable to rainbow table attacks | Private DNA (encrypted) |
| ❌ **Display Name** | Personally Identifiable Information (PII) | Private DNA (encrypted) |

**Key Insight**: v1.2 removed ALL PII from the public DHT. This DNA now only stores data that is **designed to be public** by W3C DID standards.

---

## 🛡️ Security Model

### Public by Design
- ✅ All data on this DHT is **fully public** and readable by anyone
- ✅ **No secrets** are stored here (all sensitive data is in the Private DNA)
- ✅ **Censorship-resistant** - Once published, cannot be deleted

### Critical Security Fix (November 2025)
**v1.0-v1.1 Vulnerability**: Stored `SHA-256(email)` on public DHT  
**Risk**: Rainbow table attacks could reveal user emails  
**Fix**: v1.2 completely removed email hashes from public DHT  
**Status**: All production users migrated to v1.2

---

## 📁 Repository Structure

```
flowsta-identity-dna/
├── v1.0/          # Original version (Oct 2024) - Had email_hash vulnerability
├── v1.1/          # Incremental update (Oct 2024) - Still had email_hash
├── v1.2/          # CURRENT - Security fix (Nov 2025)
│   ├── dna.yaml       # DNA configuration
│   ├── happ.yaml      # hApp bundle definition
│   ├── build.sh       # Build script
│   └── zomes/
│       ├── users/     # User profile management
│       └── sites/     # Site membership tracking
└── README.md      # This file
```

---

## 🚀 Version History

| Version | Date | Status | Changes |
|---------|------|--------|---------|
| **v1.2** | Nov 2025 | ✅ **Current** | **Security fix**: Removed `email_hash` and `display_name` from public DHT |
| v1.1 | Oct 2024 | ⚠️ Deprecated | Added site membership tracking |
| v1.0 | Oct 2024 | ⚠️ Deprecated | Initial version (email_hash vulnerability) |

**Network Seed**: `flowsta-identity-network-v1.2`  
**DNA Hash (Production)**: `uhC0k6CE2KlSn06T9Gquq_sQhf9nctsEWa6q09SWBbzdZh4ViXbB7`

---

## 🧬 DNA Entry Types

### 1. UserProfile (Public)

Stores censorship-resistant identity information.

```rust
#[hdk_entry_helper]
pub struct UserProfile {
    pub did: String,                    // W3C DID (e.g., "did:flowsta:abc123...")
    pub profile_picture: String,        // Base64-encoded image or identicon SVG
    pub has_custom_picture: bool,       // True if user uploaded custom image
    pub created_at: i64,
    pub updated_at: i64,
}
```

**Functions:**
- `register_user(profile: UserProfile)` - Create profile on DHT
- `get_my_profile()` - Retrieve current agent's profile
- `update_profile(profile: UserProfile)` - Update profile data
- `get_profile(agent: AgentPubKey)` - Get any user's profile (public)

### 2. SiteMembership (Public)

Tracks which users have joined which websites/apps.

```rust
#[hdk_entry_helper]
pub struct SiteMembership {
    pub site_id: String,               // Domain or app ID
    pub joined_at: i64,
    pub metadata: Option<String>,      // App-specific data
}
```

**Functions:**
- `join_site(site_id: String)` - Record site membership
- `get_my_sites()` - List sites current agent has joined
- `get_site_members(site_id: String)` - List all members of a site

---

## 🔧 Building from Source

### Prerequisites

- **Rust** 1.75+
- **Holochain** 0.6.0
- **Holochain CLI**: `cargo install holochain_cli`

### Build

```bash
# Clone the repository
git clone https://github.com/WeAreFlowsta/flowsta-identity-dna.git
cd flowsta-identity-dna/v1.2

# Build the DNA and hApp bundle
bash build.sh

# Output: workdir/flowsta_identity_v1_2_happ.happ
```

### Install on Conductor

```bash
# Install the hApp
hc app install ./v1.2/workdir/flowsta_identity_v1_2_happ.happ

# Or use Holochain Admin API
# (See integration guide below)
```

---

## 🔗 Integration Guide

### Installing via Holochain Admin API

```javascript
import { AdminWebsocket } from '@holochain/client';
import fs from 'fs';

// Connect to conductor
const admin = await AdminWebsocket.connect({
  url: new URL('ws://localhost:4444'),
  wsClientOptions: { origin: 'http://localhost' }
});

// Install the hApp for a new user
const appBundle = fs.readFileSync('./flowsta_identity_v1_2_happ.happ');

const installedApp = await admin.installApp({
  bundle: appBundle,
  agent_key: userAgentPubKey,  // From your authentication system
  installed_app_id: `flowsta-identity-${userId}`,
  network_seed: 'flowsta-identity-network-v1.2'
});

await admin.enableApp({ 
  installed_app_id: installedApp.installed_app_id 
});
```

### Calling Zome Functions

```javascript
import { AppWebsocket } from '@holochain/client';

// Connect to app interface
const app = await AppWebsocket.connect({
  url: new URL('ws://localhost:4444'),
  wsClientOptions: { origin: 'http://localhost' }
});

// Register a new user profile
const profile = {
  did: 'did:flowsta:abc123...',
  profile_picture: 'data:image/svg+xml;base64,...',
  has_custom_picture: false,
  created_at: Date.now(),
  updated_at: Date.now()
};

const result = await app.callZome({
  cap_secret: null,
  cell_id: [dnaHash, agentPubKey],
  zome_name: 'users',
  fn_name: 'register_user',
  payload: profile
});
```

---

## 🤝 Why Dual-DNA Architecture?

Flowsta uses **two separate DNAs** to balance censorship resistance with privacy:

| Need | DNA Used | Why |
|------|----------|-----|
| **Censorship resistance** | Identity (Public) | Site owners can't delete your DID |
| **Privacy** | [Private DNA](https://github.com/WeAreFlowsta/flowsta-private-dna) | Email, username stay encrypted |
| **Account recovery** | [Private DNA](https://github.com/WeAreFlowsta/flowsta-private-dna) | Recovery phrase never leaves user control |

**Trade-off**: We can't put sensitive data on the public DHT (privacy violation), but we also can't put DIDs on a private encrypted DHT (defeats censorship resistance). Hence: **two DNAs**.

---

## 🧪 Testing

```bash
cd v1.2

# Run Rust unit tests
cargo test

# Integration testing (requires running conductor)
# See TESTING.md for full test suite
```

---

## 🚨 Security

### Reporting Vulnerabilities

If you discover a security vulnerability, please email **security@flowsta.com** with:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

**Please do not** open a public GitHub issue for security vulnerabilities.

We aim to respond within 48 hours and will credit researchers in our security advisories.

### Security Audit History

- **November 2025**: Security review identified email_hash vulnerability → Fixed in v1.2
- **January 2026**: Production deployment with multi-node DHT

We welcome independent security audits of this code.

---

## 📚 Documentation

- **[Flowsta Auth](https://flowsta.com)** - Main website
- **[Developer Portal](https://dev.flowsta.com)** - Integration guides
- **[Private DNA](https://github.com/WeAreFlowsta/flowsta-private-dna)** - Companion DNA for encrypted data

---

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-improvement`)
3. Make your changes in the **latest version directory** (currently v1.2)
4. Test thoroughly (both unit and integration tests)
5. Submit a pull request

### Creating a New DNA Version

If you need to make breaking changes:

```bash
# Copy the latest version
cp -r v1.2 v1.3

# Update network seed in dna.yaml
# Update version in happ.yaml
# Make your changes
# Document migration path from v1.2 → v1.3
```

---

## 📝 License

Copyright © 2024-2026 Flowsta

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

**Why Apache 2.0?**
- Explicit patent grant protection
- Enterprise-friendly for commercial integrations
- Prevents patent trolling
- Consistent with Holochain's license

---

## 🙏 Acknowledgments

Built with [Holochain](https://holochain.org) - A framework for distributed applications.

Special thanks to the Holochain community for guidance on DHT architecture and security best practices.

---

## 🔗 Related Projects

- **[Flowsta Private DNA](https://github.com/WeAreFlowsta/flowsta-private-dna)** - Zero-knowledge encrypted user data
- **Flowsta Auth API** - Backend service (integration layer)
- **Flowsta Website** - User-facing application

---

**Status**: ✅ Production (v1.2)  
**Last Updated**: January 2026  
**Maintained by**: [Flowsta Team](https://flowsta.com)