#pragma once

#include <juce_dsp/juce_dsp.h>
#include <vector>
#include "EqParams.h"

class EqFilters
{
public:
    void prepare (double sampleRate, int channels);
    void reset();
    void setParams (const float values[eqparams::count]);
    float processSample (int channel, float x);
    void processChannel (int channel, float* samples, int numSamples);
    int latencySamples() const { return phaseMode == 1 ? 512 : 0; }
    float magnitudeDb (float hz) const;
    float outputDb (int channel) const;

private:
    static constexpr int maxChannels = 8;
    static constexpr int firLen = 1024;
    static constexpr int convFft = 2048;

    struct Channel
    {
        juce::dsp::IIR::Filter<float> bands[eqparams::bands];
        float convHist[convFft] {};
        int convPos = 0;
        juce::uint64 convWritten = 0;
    };

    void rebuild();
    void designLinear();
    void designAnalogZl();
    void prepareKernel();
    void firChunk (int channel, float* samples, int count);
    void pushMeter (int channel, float peak);
    bool bandIsRunning (int band) const;

    std::vector<Channel> channels;
    double sampleRate = 48000.0;
    int phaseMode = 0;
    float trimLin = 1.0f;
    float values[eqparams::count] {};
    float fir[firLen] {};
    float kernel[convFft * 2] {};
    juce::dsp::FFT fft2048 { 11 };
    std::vector<float> convWork;
    float meter[2] { -80.0f, -80.0f };
    bool ready = false;
};
