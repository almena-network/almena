# Security policy

## Supported versions

Almena is under construction and has not had a release yet: this repository is a scaffold, and
what ships today are `node` and `client`. Only the current `develop` branch receives fixes.
Once versions are published, this section will list which ones are supported.

A vulnerability in a screen that has not moved here yet belongs to the repository it still
lives in — [node](https://github.com/almena-network/node) or
[client](https://github.com/almena-network/client). Report it there, and tell us anyway if you
believe this repository will inherit it.

## Reporting a vulnerability

**Do not open a public issue, pull request or discussion for a security problem.**

Report it privately through GitHub:

<https://github.com/almena-network/almena/security/advisories/new>

That form opens a private thread visible only to you and the maintainers.

Please include, as far as you can:

- What the problem is and why you believe it is a security issue.
- The commit or branch affected, and the device or computer, the operating system and its
  version you reproduced it on.
- Steps to reproduce, ideally minimal — a proof of concept helps a great deal.
- The impact you think it has, and any mitigation you are aware of.

Almena is built so that there is nothing about a person to leak: no account, no sign-up, and
no personal data stored by a node or carried between them. A way to make this application
send, store or infer something about the person using it is a security problem of the first
order, and it is worth reporting even when nothing is technically broken.

## What happens next

- We acknowledge the report within five working days.
- We confirm or reject it, and tell you what we found.
- We agree a disclosure date with you. The default is publication once a fix is available,
  and no later than 90 days after the report.
- We credit you in the advisory unless you prefer otherwise.

Please give us a chance to fix the problem before disclosing it publicly.

## Scope

This policy covers the code in this repository. Vulnerabilities in dependencies should be
reported to their maintainers; tell us anyway if Almena is affected by one.

Since no release exists yet, all findings apply to unreleased software. That does not make
them less welcome — it is the cheapest moment to fix them.
