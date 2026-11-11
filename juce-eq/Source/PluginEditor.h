#pragma once

#include <juce_audio_processors/juce_audio_processors.h>
#include "PluginProcessor.h"

class EqAudioProcessorEditor : public juce::AudioProcessorEditor, private juce::Timer
{
public:
    explicit EqAudioProcessorEditor (EqAudioProcessor&);
    ~EqAudioProcessorEditor() override;

    void paint (juce::Graphics&) override;
    void mouseDown (const juce::MouseEvent&) override;
    void mouseDrag (const juce::MouseEvent&) override;
    void mouseUp (const juce::MouseEvent&) override;
    int controlAt (float x, float y) const;
    void moveNode (int band, float x, float y);
    void cycleType (int band);
    void selectPhase (int mode);
    void refreshSpectrum();
    float spectrumPeakDb() const;

private:
    void timerCallback() override;
    void paintSpectrum (juce::Graphics&, juce::Rectangle<float> graph);
    float uiScale() const;
    juce::Point<float> designPoint (juce::Point<float>) const;
    juce::Rectangle<float> plotBounds() const;
    juce::Rectangle<float> graphBounds() const;
    float freqToX (float freq) const;
    float xToFreq (float x) const;
    float dbToY (float db) const;
    float yToDb (float y) const;
    void setRaw (const char* id, float value);
    float raw (const char* id) const;
    int nodeAt (juce::Point<float> p) const;

    struct Fft;
    EqAudioProcessor& eq;
    std::unique_ptr<Fft> fft;
    double fftClockMs = 0.0;
    int dragNode = -1;
    int dragValue = -1;
    float dragStart = 0.0f;
    float dragOriginY = 0.0f;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (EqAudioProcessorEditor)
};
