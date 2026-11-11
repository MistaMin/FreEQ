#pragma once

#include <juce_graphics/juce_graphics.h>

namespace eqparams
{
inline constexpr int bands = 6;
inline constexpr int count = bands * 4 + 2;
inline constexpr int phaseIndex = bands * 4;
inline constexpr int trimIndex = bands * 4 + 1;

inline constexpr const char* ids[count] = {
    "b1_type", "b1_freq", "b1_gain", "b1_q",
    "b2_type", "b2_freq", "b2_gain", "b2_q",
    "b3_type", "b3_freq", "b3_gain", "b3_q",
    "b4_type", "b4_freq", "b4_gain", "b4_q",
    "b5_type", "b5_freq", "b5_gain", "b5_q",
    "b6_type", "b6_freq", "b6_gain", "b6_q",
    "phase", "trim",
};

inline constexpr const char* typeNames[5] = { "Peak", "High Pass", "Low Pass", "Low Shelf", "High Shelf" };
inline constexpr const char* phaseNames[3] = { "MP", "LP", "ZL" };

inline juce::Rectangle<float> phaseButton (int mode)
{
    return { 786.0f + (float) mode * 64.0f, 430.0f, 56.0f, 28.0f };
}

inline int typeIndex (int band) { return band * 4; }
inline int freqIndex (int band) { return band * 4 + 1; }
inline int gainIndex (int band) { return band * 4 + 2; }
inline int qIndex (int band) { return band * 4 + 3; }
} // namespace eqparams
