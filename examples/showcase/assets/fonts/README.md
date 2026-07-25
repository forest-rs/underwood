# Showcase font assets

The three `NotoSansCJK*-Regular.subset.otf` files are proof-only subsets of the
official Noto Sans CJK 2.004 release:

- upstream repository: <https://github.com/notofonts/noto-cjk>;
- release tag: `Sans2.004`;
- release commit: `523d033d6cb47f4a80c58a35753646f5c3608a78`;

| Proof asset | Upstream source path | Source SHA-256 | Subset SHA-256 |
| --- | --- | --- | --- |
| `NotoSansCJKjp-Regular.subset.otf` | `Sans/OTF/Japanese/NotoSansCJKjp-Regular.otf` | `68a3fc98800b2a27b371f2fb79991daf3633bd89309d4ffaa6946fd587f375b5` | `8da834c58f395a0d76a1e115993fd6f180d761d65562b53d8f24a3399290d3aa` |
| `NotoSansCJKsc-Regular.subset.otf` | `Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf` | `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b` | `b84cfdc16e07fea8baf3875163702b95a4f71d6bfc461b9f0cb7736f40db2c3d` |
| `NotoSansCJKkr-Regular.subset.otf` | `Sans/OTF/Korean/NotoSansCJKkr-Regular.otf` | `6bcb2a0703aa137e874fc2dffa85f6c21ba9a67fa329e81b8c801663af7e992a` | `027606dab4ac7bf5cf57f660ac0c14aecebf31e26d92cef2dea7f108d902c737` |

The subsets were produced on 2026-07-25 with HarfBuzz `hb-subset`, retaining
the exact Japanese, Simplified Chinese, and Korean text used by the living page,
plus the Simplified Chinese native-IME specimen. Original Noto family and name
records are preserved. Language-tagged styles and fallback rules select the
matching regional face instead of asking one regional Han design to stand in
for all three languages.

All three subsets remain under the SIL Open Font License 1.1. The exact license
from the pinned release is in `OFL.txt`; source copyright notices remain in the
fonts' metadata.
