# Security Policy

## Supported versions

Security fixes are applied to the latest released Phase 1 version and the `main` branch.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting for this repository (Security tab →
"Report a vulnerability"). Do not open a public issue for a suspected vulnerability.

Include:

- the affected version or commit,
- the impact and an attacker model,
- reproduction steps,
- any redacted logs that help confirm the issue.

Never include a real Discord token, cookie file contents, or unredacted personal data.

## Secrets policy

- Tokens belong in `.env` or the deployment environment, never in source control.
- `cookies.txt` and similar files must stay outside the repository; `.gitignore` already excludes them.
- Log messages must identify configuration sources by path only when safe, and must never print secret values.

## Response expectations

You should receive an acknowledgement after triage. We will coordinate disclosure and a fix with
you before publishing details. Please allow reasonable time for a patch before public disclosure.
