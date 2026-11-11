#include "PluginProcessor.h"
#include "PluginEditor.h"
#include "EqParams.h"

namespace
{
juce::AudioParameterFloat* addFreq (juce::AudioProcessorValueTreeState::ParameterLayout& layout,
                                    const char* id, const char* name, float def)
{
    layout.add (std::make_unique<juce::AudioParameterFloat> (
        juce::ParameterID { id, 1 }, name,
        juce::NormalisableRange<float> (20.0f, 20000.0f, 0.0f, 0.25f), def));
    return nullptr;
}

void addSlope (juce::AudioProcessorValueTreeState::ParameterLayout& layout,
               const char* id, const char* name)
{
    layout.add (std::make_unique<juce::AudioParameterChoice> (
        juce::ParameterID { id, 1 }, name,
        juce::StringArray { "6 dB", "12 dB", "24 dB" }, 1));
}

void addGain (juce::AudioProcessorValueTreeState::ParameterLayout& layout,
              const char* id, const char* name)
{
    layout.add (std::make_unique<juce::AudioParameterFloat> (
        juce::ParameterID { id, 1 }, name,
        juce::NormalisableRange<float> (-18.0f, 18.0f, 0.01f), 0.0f));
}

void addQ (juce::AudioProcessorValueTreeState::ParameterLayout& layout,
           const char* id, const char* name)
{
    layout.add (std::make_unique<juce::AudioParameterFloat> (
        juce::ParameterID { id, 1 }, name,
        juce::NormalisableRange<float> (0.3f, 8.0f, 0.0f, 0.5f), 0.707f));
}
} // namespace

juce::AudioProcessorValueTreeState::ParameterLayout EqAudioProcessor::createLayout()
{
    juce::AudioProcessorValueTreeState::ParameterLayout layout;
    const float freq[6] = { 50.0f, 92.0f, 500.0f, 3339.0f, 7885.0f, 10992.0f };
    const float gain[6] = { 0.0f, 0.0f, 7.9f, 2.8f, 0.0f, 0.0f };
    const float q[6] = { 0.90f, 0.98f, 0.48f, 0.80f, 0.80f, 0.90f };
    const int type[6] = { 1, 0, 0, 0, 0, 0 };
    for (int band = 0; band < 6; ++band)
    {
        const auto n = juce::String (band + 1);
        layout.add (std::make_unique<juce::AudioParameterChoice> (
            juce::ParameterID { eqparams::ids[eqparams::typeIndex (band)], 1 }, "Band " + n + " Type",
            juce::StringArray { "Peak", "High Pass", "Low Pass", "Low Shelf", "High Shelf" }, type[band]));
        layout.add (std::make_unique<juce::AudioParameterFloat> (
            juce::ParameterID { eqparams::ids[eqparams::freqIndex (band)], 1 }, "Band " + n + " Freq",
            juce::NormalisableRange<float> (20.0f, 20000.0f, 0.0f, 0.25f), freq[band]));
        layout.add (std::make_unique<juce::AudioParameterFloat> (
            juce::ParameterID { eqparams::ids[eqparams::gainIndex (band)], 1 }, "Band " + n + " Gain",
            juce::NormalisableRange<float> (-18.0f, 18.0f, 0.01f), gain[band]));
        layout.add (std::make_unique<juce::AudioParameterFloat> (
            juce::ParameterID { eqparams::ids[eqparams::qIndex (band)], 1 }, "Band " + n + " Q",
            juce::NormalisableRange<float> (0.2f, 8.0f, 0.0f, 0.5f), q[band]));
    }
    layout.add (std::make_unique<juce::AudioParameterChoice> (
        juce::ParameterID { "phase", 1 }, "Phase",
        juce::StringArray { "Minimum", "Linear", "Analog ZL" }, 0));
    layout.add (std::make_unique<juce::AudioParameterFloat> (
        juce::ParameterID { "trim", 1 }, "Trim",
        juce::NormalisableRange<float> (-24.0f, 24.0f, 0.01f), 0.0f));
    return layout;
}

EqAudioProcessor::EqAudioProcessor()
    : AudioProcessor (BusesProperties()
                          .withInput ("Input", juce::AudioChannelSet::stereo(), true)
                          .withOutput ("Output", juce::AudioChannelSet::stereo(), true)),
      apvts (*this, nullptr, "PARAMS", createLayout())
{
}

EqAudioProcessor::~EqAudioProcessor() = default;

void EqAudioProcessor::pushParamsToFilters()
{
    float values[eqparams::count];
    for (int i = 0; i < eqparams::count; ++i)
        values[i] = apvts.getRawParameterValue (eqparams::ids[i])->load();
    filters.setParams (values);
    setLatencySamples (filters.latencySamples());
}

void EqAudioProcessor::prepareToPlay (double sampleRate, int samplesPerBlock)
{
    juce::ignoreUnused (samplesPerBlock);
    filters.prepare (sampleRate, juce::jmax (1, getTotalNumOutputChannels()));
    fftRing.reset();
    pushParamsToFilters();
}

void EqAudioProcessor::releaseResources()
{
    filters.reset();
}

void EqAudioProcessor::reset()
{
    filters.reset();
}

bool EqAudioProcessor::isBusesLayoutSupported (const BusesLayout& layouts) const
{
    return layouts.getMainOutputChannelSet() == juce::AudioChannelSet::stereo()
        && layouts.getMainInputChannelSet() == juce::AudioChannelSet::stereo();
}

void EqAudioProcessor::processBlock (juce::AudioBuffer<float>& buffer, juce::MidiBuffer& midi)
{
    juce::ScopedNoDenormals noDenormals;
    juce::ignoreUnused (midi);
    pushParamsToFilters();

    for (int channel = 0; channel < buffer.getNumChannels(); ++channel)
    {
        filters.processChannel (channel, buffer.getWritePointer (channel), buffer.getNumSamples());
    }
    if (spectrumOn)
    {
        const int channels = juce::jmin (buffer.getNumChannels(), 8);
        const float* pointers[8] {};
        for (int channel = 0; channel < channels; ++channel)
            pointers[channel] = buffer.getReadPointer (channel);
        fftRing.push (pointers, channels, buffer.getNumSamples());
    }
}

juce::AudioProcessorEditor* EqAudioProcessor::createEditor()
{
    return new EqAudioProcessorEditor (*this);
}

void EqAudioProcessor::getStateInformation (juce::MemoryBlock& destData)
{
    if (auto state = apvts.copyState(); state.isValid())
    {
        juce::MemoryOutputStream stream (destData, false);
        state.writeToStream (stream);
    }
}

void EqAudioProcessor::setStateInformation (const void* data, int sizeInBytes)
{
    if (auto tree = juce::ValueTree::readFromData (data, static_cast<size_t> (sizeInBytes)); tree.isValid())
        apvts.replaceState (tree);
}

juce::AudioProcessor* JUCE_CALLTYPE createPluginFilter()
{
    return new EqAudioProcessor();
}
