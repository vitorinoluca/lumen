# Data sources

lumen analyzes everything locally. The only network use is downloading public, free data, and only when
you press the button. Every source is reviewed for license and terms before it is integrated. A source
that requires registration or an API key is rejected.

## Used

| Source | License | What lumen uses | When it is downloaded |
|---|---|---|---|
| [LOLDrivers](https://www.loldrivers.io) | Apache-2.0, free download, no key | SHA-256 hashes of known vulnerable or malicious drivers, with their category. Only the hashes are kept (about 160 KB) in `%APPDATA%\lumen\db`. | Only when you press "Actualizar hashes". |

Attribution: this product contains data from LOLDrivers (https://github.com/magicsword-io/LOLDrivers),
licensed under the Apache License 2.0.

## Evaluated and rejected

| Source | Why |
|---|---|
| abuse.ch MalwareBazaar | Its API requires an `Auth-Key` (registration) and works under "fair use", with a paid subscription possible for commercial use. It is not CC0. |

## Pending review

YARA rules (for example YARA-Forge) and ClamAV signatures: license review comes first, as in the plan.
