# Model and parity inputs

This folder holds the Apollo model for Melomae. The model is not built here.
`tools/export` builds it from the pinned checkpoint.

## The Apollo model

| Item | Value |
| --- | --- |
| File | `models/apollo.onnx` |
| Size | 77,649,552 bytes |
| SHA-256 | `fdf62f6c0ba5daf3a38c1761443456e4656707218b91a19cdad41a5d4bad78b9` |
| Source repository | <https://github.com/JusperLee/Apollo> |
| Checkpoint | `pytorch_model.bin`, 66,541,845 bytes |
| Checkpoint SHA-256 | `99d9af7f1ff20e63c393035513a655392818d66b4d7fc23d658175c1f15e8d76` |
| License | Creative Commons Attribution ShareAlike 4.0 International (CC BY-SA 4.0) |

Melomae converts the Apollo checkpoint to ONNX with `tools/export`. The
conversion puts the STFT and the iSTFT inside the graph and changes no other
model math. This is the only modification.

## Attribution

Apollo is by Kai Li and Yi Luo. Kai Li is at Tsinghua University, Beijing,
China and at Tencent AI Lab, Shenzhen, China. Yi Luo is at Tencent AI Lab,
Shenzhen, China. The Look2Hear group at Tsinghua University develops Apollo.

Cite this paper for Apollo:

> Li, Kai and Luo, Yi. "Apollo: Band-sequence Modeling for High-Quality Audio
> Restoration." ICASSP, 2025. arXiv:2409.08514.

The Apollo code and weights use CC BY-SA 4.0. The repository `LICENSE` file and
the Hugging Face model card both state that license. There is no NonCommercial
term.

Melomae does not imply that the Apollo authors, Tsinghua University, or Tencent
AI Lab endorse Melomae.

## The parity input

| Item | Value |
| --- | --- |
| File | `tools/parity/input_wav.wav` |
| Size | 2,116,858 bytes |
| SHA-256 | `3c9a053913c3016b493ddc0b92e21a4e682e8fb5f872dafc945154155bddf771` |
| Source | `JusperLee/Apollo`, first commit `d72b12c` on 2024-09-07, still present at `e84bcac` |
| License | CC BY-SA 4.0, as part of the Apollo repository |

The file is a stereo 44.1 kHz 32-bit float recording. The task 04 parity test
and `tools/parity/make_golden.py` both build an 18 s input from three copies of
it. Separate rights in the recording were not confirmed. If the Apollo
repository did not hold those rights, this file would need a different source.
