# Ember Protocol: Product Direction

This note records the product intent for future design, frontend, documentation, and implementation work.

## Identity and positioning

- **Product name:** Ember Protocol.
- **Positioning:** An autonomous emergency backup plan for multi-chain assets.
- Keep the product welcoming and neutral. It is for backup planning around lost access, emergencies, long absences, and other unexpected events. Do not make death or bereavement the obvious or dominant framing.
- The ember metaphor is a living spark kept active through periodic check-ins. If check-ins stop and the configured grace period passes, the plan can release assets to the user's selected destinations.
- The product should feel warm and dependable, not morbid or corporate.

## Destination options

Support two kinds of destination in the product model and explain them neutrally:

1. **Trusted people:** one or more people selected by the owner, with configurable shares.
2. **Personal backup wallet:** the owner’s own secondary wallet, including a full-share backup option.

Do not use "heir" in user-facing UI copy. Prefer terms such as "trusted people," "destinations," or "people you choose," depending on the screen. Keep contract/API terminology in code and technical details where necessary.

## Intended delivery experience

- The primary experience is automatic distribution after the check-in and grace conditions are met, when keeper automation is configured and operational.
- Destinations should receive native BTC and ETH in their regular wallets; they should not need to operate the NEAR contract for the primary path.
- Keep a manual claim path as a fallback where supported. The UI must plainly explain any wallet or account requirements for that fallback.
- Clearly distinguish configured automation from automation that is currently running. Never imply that a transfer happened until the available evidence supports the displayed status.

## Make NEAR's role visible

NEAR Chain Signatures and threshold MPC are central to the product and should be explained prominently in the UI and project materials. Show how a NEAR contract coordinates signing for native Bitcoin and Ethereum destinations, and why this enables one plan to cover multiple chains.

Avoid unsupported absolutes in product claims. Statements about key handling, security, supported networks, address derivation, signing, and delivery must be checked against the implemented code and live capabilities before being presented as facts.

## Intended frontend architecture

- Target a browser-first frontend using Vite, React, and JavaScript.
- The intended design avoids a project-operated backend for ordinary wallet connection, contract reads/writes, public chain data, and transaction broadcasting.
- Treat browser RPC and public explorer/indexer APIs as dependencies to validate for CORS, availability, rate limits, and privacy implications.
- Keep contract integration behind a dedicated frontend adapter so UI code is not coupled directly to RPC or contract argument details.

## Language and interaction principles

- Use neutral backup-plan language throughout the UI.
- Prefer "Check in," "Withdraw," "Personal backup wallet," "Trusted people," and "Ready for distribution" over internal method or lifecycle names.
- Explain irreversible actions before they are submitted and show network, destination, fees, and transaction state where relevant.
- Distinguish a transaction being prepared, broadcast, confirmed by a public source, or merely reported by a participant. Use "Delivered (reported)" when delivery is based on an unverified report.
- Do not show controls for features the current contract or deployed version does not support. Mark future features as unavailable or omit them until implemented and checked.
- Do not use "heir" in user-facing copy. Keep technical contract names where needed for code, logs, and developer documentation.
