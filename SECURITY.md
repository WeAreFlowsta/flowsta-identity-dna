# Security Policy

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please email security@flowsta.com with:

1. **Description** - Detailed explanation of the vulnerability
2. **Steps to Reproduce** - How to trigger the issue
3. **Impact** - Potential consequences (data leakage, DOS, etc.)
4. **Suggested Fix** - If you have one (optional)
5. **Your Contact Info** - For follow-up questions

### What to Expect

- **Initial Response**: Within 48 hours
- **Status Update**: Within 1 week
- **Fix Timeline**: Depends on severity (critical issues prioritized)
- **Credit**: We'll credit you in our security advisories (unless you prefer anonymity)

## Supported Versions

| Version | Status | Support |
|---------|--------|---------|
| v1.2 | ✅ Production | Actively maintained |
| v1.1 | ⚠️ Deprecated | Security fixes only |
| v1.0 | ❌ End of Life | No support |

## Security Model

### Public by Design

The **Flowsta Identity DNA** stores data on a **public DHT** by design. This means:

- ✅ All data is **fully readable by anyone** on the DHT
- ✅ **No secrets** are stored (all sensitive data is in the [Private DNA](https://github.com/WeAreFlowsta/flowsta-private-dna))
- ✅ **Censorship-resistant** - Once published, data cannot be deleted

### What's Safe to Store

✅ **Safe** (designed to be public):
- W3C DIDs (Decentralized Identifiers)
- Profile pictures (avatars/identicons)
- Site membership records

❌ **Never Store** (use Private DNA instead):
- Email addresses or email hashes
- Display names
- Personal Identifiable Information (PII)
- Authentication credentials
- Recovery phrases

### Critical Security Fix (v1.2)

**Issue**: v1.0 and v1.1 stored `SHA-256(email)` on the public DHT, which was vulnerable to rainbow table attacks.

**Fix**: v1.2 completely removed `email_hash` and `display_name` from the public DHT schema.

**Status**: All production users migrated to v1.2.

## Known Security Considerations

### 1. DHT Data is Permanent

Once data is published to the DHT, it **cannot be fully deleted**. Other nodes may retain copies. Design accordingly:
- Only store data intended to be public
- Never store sensitive information (even hashed)
- Use the Private DNA for anything private

### 2. Profile Pictures

Profile pictures are stored as base64-encoded images (< 200KB). While public, this is by design:
- Users choose their own avatars
- Identicons are generated from public DIDs
- No PII is embedded in images

### 3. Agent Keys

Holochain agent keys (public keys) are visible on the DHT. This is expected and does not compromise security:
- They are designed to be public
- They're used for signing DHT operations
- Private keys are stored securely in Lair keystore

## Disclosure Policy

When we receive a security report:

1. **Acknowledge** - Confirm receipt within 48 hours
2. **Investigate** - Assess severity and impact
3. **Fix** - Develop and test a patch
4. **Notify** - Inform affected users (if applicable)
5. **Publish** - Release a security advisory

We follow **responsible disclosure**:
- We'll work with you to understand the issue
- We won't publicly disclose until a fix is available
- We'll credit you in our advisory (unless you prefer anonymity)

## Security Best Practices for Integrators

If you're integrating this DNA into your application:

### ✅ Do:
- Validate all data before storing on DHT
- Only store public information
- Use the Private DNA for sensitive data
- Implement rate limiting on write operations
- Monitor DHT for malicious patterns

### ❌ Don't:
- Store PII on the public DHT
- Hash emails and store them (still vulnerable)
- Embed secrets in profile pictures
- Trust DHT data without validation
- Use v1.0 or v1.1 (deprecated due to security issues)

## Vulnerability Examples

### High Severity
- Authentication bypass
- Ability to impersonate other users
- DOS attacks that prevent DHT operations
- Data injection attacks

### Medium Severity
- Information disclosure beyond public DHT design
- Validation bypass
- Improper error handling leaking internal state

### Low Severity
- Documentation errors
- Non-exploitable edge cases
- Performance issues without security impact

## Security Audit History

- **November 2025**: Internal review → Identified email_hash vulnerability → Fixed in v1.2
- **January 2026**: Production deployment review → Multi-node DHT security assessment

## Bug Bounty

We currently don't have a formal bug bounty program, but we deeply appreciate security research and will:
- Credit you in our security advisories
- Consider compensation for critical vulnerabilities (case-by-case)
- Fast-track your contributions

## Contact

**Security Email**: security@flowsta.com  
**PGP Key**: (Coming soon)  
**Response Time**: 48 hours

---

**Last Updated**: January 2026  
**Maintained by**: [Flowsta Security Team](https://flowsta.com)
