"""Real-arithmetic STFT and iSTFT for the ONNX Apollo export.

The upstream Apollo model uses ``torch.stft`` and ``torch.istft`` with complex
tensors. ONNX has no complex type and no iSTFT operator. This module expresses
both transforms with real arithmetic only:

* the forward STFT is a ``conv1d`` over a reflect-padded signal, with a
  cosine/sine DFT basis and a periodic Hann window,
* the inverse STFT is a ``conv_transpose1d`` overlap-add, normalized by the
  window envelope.

The model mathematics does not change. Only the STFT and iSTFT change their
form.
"""

import math

import torch
import torch.nn as nn
import torch.nn.functional as F


def analysis_kernels(win: int, enc_dim: int, dtype: torch.dtype):
    """Return the cosine and sine analysis kernels, shape ``(enc_dim, win)``.

    The basis is built in float64 with a reduced angle, then cast to ``dtype``.
    A float32 angle spans thousands of radians and loses precision. The
    reduced angle stays below ``2*pi``.
    """
    n = torch.arange(win, dtype=torch.float64)
    k = torch.arange(enc_dim, dtype=torch.float64).unsqueeze(1)
    window = torch.hann_window(win, periodic=True, dtype=torch.float64)
    angle = 2.0 * math.pi * ((k * n) % win) / win
    cos_kernel = (torch.cos(angle) * window).to(dtype)
    sin_kernel = (-torch.sin(angle) * window).to(dtype)
    return cos_kernel, sin_kernel


def synthesis_kernels(win: int, enc_dim: int, dtype: torch.dtype):
    """Return the cosine/sine synthesis kernels and the squared-window kernel.

    The kernels include the inverse-DFT scale and the synthesis window. The
    DC and Nyquist bins take half the weight of the interior bins, because the
    one-sided spectrum is Hermitian.
    """
    n = torch.arange(win, dtype=torch.float64)
    k = torch.arange(enc_dim, dtype=torch.float64).unsqueeze(1)
    window = torch.hann_window(win, periodic=True, dtype=torch.float64)
    scale = torch.full((enc_dim, 1), 2.0 / win, dtype=torch.float64)
    scale[0] = 1.0 / win
    scale[enc_dim - 1] = 1.0 / win
    angle = 2.0 * math.pi * ((k * n) % win) / win
    cos_kernel = (scale * torch.cos(angle) * window).to(dtype)
    sin_kernel = (-scale * torch.sin(angle) * window).to(dtype)
    envelope_kernel = (window * window).to(dtype).reshape(1, 1, win)
    return cos_kernel, sin_kernel, envelope_kernel


class RealApollo(nn.Module):
    """Apollo with a real-arithmetic STFT and iSTFT.

    The instance reuses the loaded ``BN``, ``net``, and ``output`` submodules.
    The forward signature matches ``Apollo.forward``: ``(batch, channels,
    samples)`` to ``(batch, channels, samples)``.
    """

    def __init__(self, apollo):
        super().__init__()
        self.BN = apollo.BN
        self.net = apollo.net
        self.output = apollo.output
        self.win = int(apollo.win)
        self.stride = int(apollo.stride)
        self.enc_dim = int(apollo.enc_dim)
        self.band_width = [int(w) for w in apollo.band_width]
        self.eps = float(apollo.eps)

        cos_kernel, sin_kernel = analysis_kernels(self.win, self.enc_dim, torch.float32)
        syn_cos, syn_sin, envelope = synthesis_kernels(self.win, self.enc_dim, torch.float32)
        self.register_buffer("stft_cos", cos_kernel.reshape(self.enc_dim, 1, self.win))
        self.register_buffer("stft_sin", sin_kernel.reshape(self.enc_dim, 1, self.win))
        self.register_buffer("istft_cos", syn_cos.reshape(self.enc_dim, 1, self.win))
        self.register_buffer("istft_sin", syn_sin.reshape(self.enc_dim, 1, self.win))
        self.register_buffer("window_envelope", envelope, persistent=False)

    def forward(self, input: torch.Tensor) -> torch.Tensor:
        batch, channels, samples = input.shape
        folded = input.reshape(batch * channels, samples)

        pad = self.win // 2
        padded = F.pad(folded, (pad, pad), mode="reflect")
        frames = padded.unsqueeze(1)
        real = F.conv1d(frames, self.stft_cos, stride=self.stride)
        imag = F.conv1d(frames, self.stft_sin, stride=self.stride)

        band_features = []
        index = 0
        for width in self.band_width:
            band_real = real[:, index : index + width]
            band_imag = imag[:, index : index + width]
            power = torch.sqrt(
                (band_real * band_real + band_imag * band_imag).sum(1, keepdim=True)
                + self.eps
            )
            band_features.append(
                torch.cat([band_real / power, band_imag / power, torch.log(power)], dim=1)
            )
            index += width

        stacked = torch.stack(
            [norm(feature) for norm, feature in zip(self.BN, band_features)], dim=1
        )
        net_out = self.net(stacked)

        out_real = []
        out_imag = []
        for index, width in enumerate(self.band_width):
            estimate = self.output[index](net_out[:, index])
            estimate = estimate.view(batch * channels, 2, width, -1)
            out_real.append(estimate[:, 0])
            out_imag.append(estimate[:, 1])
        full_real = torch.cat(out_real, 1)
        full_imag = torch.cat(out_imag, 1)

        frame_count = full_real.shape[-1]
        ones = torch.ones(1, 1, frame_count, dtype=full_real.dtype, device=full_real.device)
        envelope = F.conv_transpose1d(ones, self.window_envelope, stride=self.stride)
        signal = F.conv_transpose1d(full_real, self.istft_cos, stride=self.stride)
        signal = signal + F.conv_transpose1d(full_imag, self.istft_sin, stride=self.stride)
        signal = signal / envelope.clamp(min=1e-11)
        signal = signal[..., pad : pad + samples]
        return signal.reshape(batch, channels, samples)
