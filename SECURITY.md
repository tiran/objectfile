# Security Policy

## Supported Versions

`objectfile` is pre-1.0; only the latest released version receives security fixes.

| Version | Supported          |
| ------- | ------------------ |
| latest  | :white_check_mark: |
| older   | :x:                |

## Reporting a Vulnerability

Please report security issues privately via GitHub's
[private vulnerability reporting](https://github.com/tiran/objectfile/security/advisories/new)
("Report a vulnerability" under the Security tab). Do not open a public issue for
security reports.

We aim to acknowledge reports within a few business days. `objectfile` parses untrusted
binary input; parsing bugs that lead to crashes or memory unsafety in the underlying
`object` crate are in scope and will also be reported upstream where appropriate.
