# Apollo model notice

`apollo.onnx` is the Melomae restoration model. Melomae derives it from the
Apollo model. This file gives the attribution that the model license requires
(SRS section 11).

## Attribution

Kai Li and Yi Luo created the Apollo model. Kai Li is at Tsinghua University,
Beijing, China, and at Tencent AI Lab, Shenzhen, China. Yi Luo is at Tencent AI
Lab, Shenzhen, China. The Look2Hear group at Tsinghua University develops the
project.

- Paper title: Apollo: Band-sequence Modeling for High-Quality Audio Restoration
- Paper title in the citation: Apollo: Band-sequence Modeling for High-Quality Music Restoration in Compressed Audio
- Authors: Kai Li, Yi Luo
- Preprint: https://arxiv.org/abs/2409.08514
- Source code: https://github.com/JusperLee/Apollo
- Weights: `JusperLee/Apollo` on Hugging Face, revision `c68bd80f`

## License

Apollo code and weights are licensed under the Creative Commons
Attribution-ShareAlike 4.0 International license (CC BY-SA 4.0). `apollo.onnx`
adapts the weights, so it stays under CC BY-SA 4.0.

- License: Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)
- Link: https://creativecommons.org/licenses/by-sa/4.0/legalcode

Upstream copyright notice: the Licensor supplied none with the model. The
repository `LICENSE` holds the CC BY-SA 4.0 text, and the model card names the
creators but carries no copyright line. The only copyright notice in the
repository is `Copyright 2019 Jian Wu` in `look2hear/utils/stft.py`, and that one
file licenses under Apache-2.0 rather than under CC BY-SA 4.0.

The model comes with no warranty. CC BY-SA 4.0 section 5 (Disclaimer of
Warranties and Limitation of Liability) offers the Licensed Material as-is and
as-available, and excludes the Licensor's liability for damages arising from its
use.

## Modifications

Melomae changed the model as follows:

- Melomae converted the PyTorch checkpoint to ONNX.
- Melomae re-expressed the STFT and the iSTFT as real arithmetic.

No other change applies. The model keeps the Apollo weights.

## No endorsement

Melomae does not state or imply that the Apollo authors, Tsinghua University, or
Tencent AI Lab endorse Melomae (CC BY-SA 4.0 section 2(a)(6)).

## Sources

Melomae read these sources for the facts above:

- `README.md` in the Apollo repository, at the source URL above. It names the
  creators, the paper, the citation, and the license link.
- `LICENSE` in the Apollo repository. It holds the CC BY-SA 4.0 text.
- The Hugging Face model card for `JusperLee/Apollo`. It states
  `license: cc-by-sa-4.0`.
