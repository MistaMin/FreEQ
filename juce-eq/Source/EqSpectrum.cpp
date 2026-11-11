#include "EqSpectrum.h"
#include <cstring>

SpectrumRing::SpectrumRing()
    : samples (std::make_unique<std::atomic<uint32_t>[]> ((size_t) capacity))
{
    reset();
}

void SpectrumRing::reset() noexcept
{
    for (int i = 0; i < capacity; ++i)
        samples[(size_t) i].store (0, std::memory_order_relaxed);
    writePos.store (0, std::memory_order_release);
}

void SpectrumRing::push (const float* const* channels, int numChannels, int numSamples) noexcept
{
    if (channels == nullptr || numChannels <= 0 || numSamples <= 0)
        return;
    const auto start = writePos.load (std::memory_order_relaxed);
    const float inv = 1.0f / (float) numChannels;
    for (int i = 0; i < numSamples; ++i)
    {
        float sum = 0.0f;
        for (int channel = 0; channel < numChannels; ++channel)
            if (channels[channel] != nullptr)
                sum += channels[channel][i];
        const auto index = (size_t) ((start + (uint64_t) i) & (uint64_t) (capacity - 1));
        const float mixed = sum * inv;
        uint32_t bits = 0;
        std::memcpy (&bits, &mixed, sizeof (bits));
        samples[index].store (bits, std::memory_order_relaxed);
    }
    writePos.store (start + (uint64_t) numSamples, std::memory_order_release);
}

bool SpectrumRing::copyLatest (float* dest, int count) const noexcept
{
    if (dest == nullptr || count <= 0 || count > capacity / 2)
        return false;
    const auto end = writePos.load (std::memory_order_acquire);
    if (end < (uint64_t) count)
        return false;
    const auto start = end - (uint64_t) count;
    for (int i = 0; i < count; ++i)
    {
        const auto index = (size_t) ((start + (uint64_t) i) & (uint64_t) (capacity - 1));
        const uint32_t bits = samples[index].load (std::memory_order_relaxed);
        float sample = 0.0f;
        std::memcpy (&sample, &bits, sizeof (sample));
        dest[i] = sample;
    }
    return writePos.load (std::memory_order_acquire) - start <= (uint64_t) capacity - 64;
}

uint64_t SpectrumRing::position() const noexcept
{
    return writePos.load (std::memory_order_acquire);
}
