# Vendored Apollo sources

This directory holds the upstream Apollo model sources. The ONNX export in
`tools/export/export.py` imports them unchanged.

## Source

| Item | Value |
| --- | --- |
| Repository | https://github.com/JusperLee/Apollo |
| Commit | `e84bcacc59d5455f05d86a5c97dd4aeb3c14dbb6` |
| Router model | `JusperLee/Apollo` at revision `c68bd80fdd9c0d93d2f4a833cb154624f660a561` |
| Checkpoint | `pytorch_model.bin`, 66,541,845 bytes, SHA-256 `99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76` |
| License | CC BY-SA 4.0 |

## File checksums

| File | SHA-256 |
| --- | --- |
| `look2hear/models/apollo.py` | `e34d96c2f0320f34f1297023652b741b7e5e1028fe12a946cbf9faf95f442e89` |
| `look2hear/models/base_model.py` | `8fc90f5af762ae1d0d26cc51ec03ef4a2f956483d63d72e0d39b040f90e7c598` |

The two files come from the upstream commit above. This repository does not
change them. The `__init__.py` files are Melomae shims. They make the vendored
package importable.

## License notice

Apollo is licensed under the Creative Commons Attribution-ShareAlike 4.0
International License (CC BY-SA 4.0). The license text is at
https://creativecommons.org/licenses/by-sa/4.0/legalcode.

Melomae modifies Apollo only by converting the model to ONNX. The model
mathematics does not change. The ONNX export replaces the complex-valued STFT
and iSTFT with equivalent real arithmetic. SRS §11 holds the full attribution
policy.
