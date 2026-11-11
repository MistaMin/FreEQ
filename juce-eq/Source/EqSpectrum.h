#pragma once

#include <juce_core/juce_core.h>
#include <atomic>
#include <cstdint>
#include <memory>

// Lock-free mono ring. The audio thread only calls push. The editor copies a frame and runs the FFT.
class SpectrumRing
{
public:
    static constexpr int capacity = 1 << 15;
    static constexpr int fftSize = 4096;

    SpectrumRing();
    void reset() noexcept;
    void push (const float* const* channels, int numChannels, int numSamples) noexcept;
    bool copyLatest (float* dest, int count) const noexcept;
    uint64_t position() const noexcept;

private:
    std::unique_ptr<std::atomic<uint32_t>[]> samples;
    std::atomic<uint64_t> writePos { 0 };
};
