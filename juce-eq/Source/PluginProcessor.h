#pragma once

#include <juce_audio_processors/juce_audio_processors.h>
#include "EqFilters.h"
#include "EqSpectrum.h"

class EqAudioProcessor : public juce::AudioProcessor
{
public:
    EqAudioProcessor();
    ~EqAudioProcessor() override;

    void prepareToPlay (double sampleRate, int samplesPerBlock) override;
    void releaseResources() override;
    void reset() override;
    bool isBusesLayoutSupported (const BusesLayout& layouts) const override;
    void processBlock (juce::AudioBuffer<float>&, juce::MidiBuffer&) override;

    juce::AudioProcessorEditor* createEditor() override;
    bool hasEditor() const override { return true; }

    const juce::String getName() const override { return "EQ Benchmark JUCE"; }
    bool acceptsMidi() const override { return false; }
    bool producesMidi() const override { return false; }
    bool isMidiEffect() const override { return false; }
    double getTailLengthSeconds() const override { return 0.0; }

    int getNumPrograms() override { return 1; }
    int getCurrentProgram() override { return 0; }
    void setCurrentProgram (int) override {}
    const juce::String getProgramName (int) override { return {}; }
    void changeProgramName (int, const juce::String&) override {}

    void getStateInformation (juce::MemoryBlock& destData) override;
    void setStateInformation (const void* data, int sizeInBytes) override;

    juce::AudioProcessorValueTreeState apvts;
    const SpectrumRing& spectrumRing() const { return fftRing; }
    void setSpectrumEnabled (bool enabled) { spectrumOn = enabled; }
    float magnitudeDb (float hz) const { return filters.magnitudeDb (hz); }
    float outputDb (int channel) const { return filters.outputDb (channel); }

private:
    EqFilters filters;
    SpectrumRing fftRing;
    bool spectrumOn = true;
    void pushParamsToFilters();

    static juce::AudioProcessorValueTreeState::ParameterLayout createLayout();
    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (EqAudioProcessor)
};
