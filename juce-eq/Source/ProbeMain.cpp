#include "PluginProcessor.h"
#include "PluginEditor.h"
#include "EqParams.h"

#include <algorithm>
#include <cmath>
#include <ctime>
#include <iostream>
#include <string>
#include <vector>

namespace
{
constexpr double kSampleRate = 48000.0;
constexpr float kPeak = 0.25f;
constexpr int kBlock = 512;

void setParam (EqAudioProcessor& processor, const char* id, float value)
{
    auto* parameter = dynamic_cast<juce::RangedAudioParameter*> (processor.apvts.getParameter (id));
    if (parameter == nullptr)
    {
        std::cerr << "missing parameter " << id << "\n";
        std::exit (1);
    }
    parameter->setValueNotifyingHost (parameter->convertTo0to1 (value));
}

void flat (EqAudioProcessor& processor)
{
    for (int band = 0; band < eqparams::bands; ++band)
    {
        setParam (processor, eqparams::ids[eqparams::typeIndex (band)], 0.0f);
        setParam (processor, eqparams::ids[eqparams::gainIndex (band)], 0.0f);
        setParam (processor, eqparams::ids[eqparams::qIndex (band)], 0.707f);
    }
    setParam (processor, "phase", 0.0f);
    setParam (processor, "trim", 0.0f);
}

double levelDb (EqAudioProcessor& processor, float hz)
{
    processor.reset();
    juce::AudioBuffer<float> buffer (2, kBlock);
    juce::MidiBuffer midi;
    double sumSq = 0.0;
    int counted = 0;
    int produced = 0;
    float phase = 0.0f;
    const float step = hz / static_cast<float> (kSampleRate);
    while (counted < 8192)
    {
        for (int i = 0; i < kBlock; ++i)
        {
            phase = std::fmod (phase + step, 1.0f);
            const float sample = std::sin (phase * juce::MathConstants<float>::twoPi) * kPeak;
            buffer.setSample (0, i, sample);
            buffer.setSample (1, i, sample);
        }
        processor.processBlock (buffer, midi);
        for (int i = 0; i < kBlock; ++i)
        {
            if (produced >= 8192 && counted < 8192)
            {
                const double y = buffer.getSample (0, i);
                sumSq += y * y;
                ++counted;
            }
            ++produced;
        }
    }
    const double rmsOut = std::sqrt (sumSq / static_cast<double> (counted));
    const double rmsIn = static_cast<double> (kPeak) / std::sqrt (2.0);
    if (rmsOut < 1.0e-12)
        return -120.0;
    return 20.0 * std::log10 (rmsOut / rmsIn);
}

void expect (bool ok, const std::string& message)
{
    if (! ok)
    {
        std::cerr << "FAIL " << message << "\n";
        std::exit (1);
    }
}

void checkTypes()
{
    EqAudioProcessor highpass;
    highpass.prepareToPlay (kSampleRate, kBlock);
    flat (highpass);
    setParam (highpass, "b1_type", 1.0f);
    setParam (highpass, "b1_freq", 400.0f);
    setParam (highpass, "b1_q", 0.707f);
    const double low = levelDb (highpass, 80.0f);
    const double high = levelDb (highpass, 2000.0f);
    std::cout << "PROBE case=highpass low=" << low << " high=" << high << "\n";
    expect (low < high - 12.0, "highpass");

    EqAudioProcessor lowpass;
    lowpass.prepareToPlay (kSampleRate, kBlock);
    flat (lowpass);
    setParam (lowpass, "b1_type", 2.0f);
    setParam (lowpass, "b1_freq", 1000.0f);
    const double passed = levelDb (lowpass, 200.0f);
    const double cut = levelDb (lowpass, 8000.0f);
    std::cout << "PROBE case=lowpass pass=" << passed << " cut=" << cut << "\n";
    expect (cut < passed - 18.0, "lowpass");

    EqAudioProcessor peak;
    peak.prepareToPlay (kSampleRate, kBlock);
    flat (peak);
    setParam (peak, "b3_freq", 1000.0f);
    setParam (peak, "b3_q", 2.0f);
    setParam (peak, "b3_gain", 12.0f);
    const double boosted = levelDb (peak, 1000.0f);
    std::cout << "PROBE case=peak level_db=" << boosted << "\n";
    expect (std::abs (boosted - 12.0) < 2.0, "peak");

    EqAudioProcessor shelf;
    shelf.prepareToPlay (kSampleRate, kBlock);
    flat (shelf);
    setParam (shelf, "b6_type", 4.0f);
    setParam (shelf, "b6_freq", 4000.0f);
    setParam (shelf, "b6_gain", 6.0f);
    const double air = levelDb (shelf, 12000.0f);
    const double body = levelDb (shelf, 200.0f);
    std::cout << "PROBE case=highshelf air=" << air << " body=" << body << "\n";
    expect (air > 3.0, "high shelf");
    expect (std::abs (body) < 1.5, "shelf leaves lows");

    setParam (peak, "b3_freq", 20.0f);
    const double at20 = levelDb (peak, 20.0f);
    setParam (peak, "b3_freq", 20000.0f);
    const double at20k = levelDb (peak, 20000.0f);
    std::cout << "PROBE case=band_ends at20=" << at20 << " at20k=" << at20k << "\n";
    expect (at20 > 6.0 && at20k > 4.0, "frequency reaches both ends");
}

void checkPhase()
{
    EqAudioProcessor linear;
    linear.prepareToPlay (kSampleRate, kBlock);
    flat (linear);
    setParam (linear, "phase", 1.0f);
    setParam (linear, "b3_freq", 1000.0f);
    setParam (linear, "b3_gain", 6.0f);
    const double level = levelDb (linear, 1000.0f);
    std::cout << "PROBE case=linear level_db=" << level << " latency=" << linear.getLatencySamples() << "\n";
    expect (std::abs (level - 6.0) < 1.5, "linear boost");
    expect (linear.getLatencySamples() == 512, "linear latency");

    EqAudioProcessor analog;
    analog.prepareToPlay (kSampleRate, kBlock);
    flat (analog);
    setParam (analog, "phase", 2.0f);
    setParam (analog, "b3_freq", 1000.0f);
    setParam (analog, "b3_q", 1.0f);
    setParam (analog, "b3_gain", 6.0f);
    const double zl = levelDb (analog, 1000.0f);
    std::cout << "PROBE case=analog_zl level_db=" << zl << " latency=" << analog.getLatencySamples() << "\n";
    expect (std::abs (zl - 6.0) < 1.5, "analog zl boost");
    expect (analog.getLatencySamples() == 0, "analog zl latency");
}

void editorTree (int argc, char** argv)
{
    juce::ScopedJuceInitialiser_GUI library;
    EqAudioProcessor processor;
    processor.prepareToPlay (kSampleRate, kBlock);
    juce::AudioBuffer<float> noise (2, kBlock);
    juce::MidiBuffer midi;
    for (int block = 0; block < 16; ++block)
    {
        for (int i = 0; i < kBlock; ++i)
        {
            const float n = (float) (block * kBlock + i);
            const float sample = std::sin (n * 0.13f) * 0.2f + std::sin (n * 0.05f) * 0.12f;
            noise.setSample (0, i, sample);
            noise.setSample (1, i, sample);
        }
        processor.processBlock (noise, midi);
    }
    std::unique_ptr<juce::AudioProcessorEditor> editor (processor.createEditor());
    expect (editor != nullptr, "editor exists");
    expect (editor->getWidth() == 980 && editor->getHeight() == 640, "editor size");
    auto* face = dynamic_cast<EqAudioProcessorEditor*> (editor.get());
    expect (face != nullptr, "editor type");
    face->refreshSpectrum();
    const float fftPeak = face->spectrumPeakDb();
    std::cout << "PROBE case=fft peak_db=" << fftPeak << "\n";
    expect (fftPeak > -40.0f, "fft sees the test tone");
    const float nodeX = 400.0f;
    const float nodeY = 180.0f;
    face->moveNode (2, nodeX, nodeY);
    const float freq = processor.apvts.getRawParameterValue ("b3_freq")->load();
    const float gain = processor.apvts.getRawParameterValue ("b3_gain")->load();
    std::cout << "PROBE case=drag_node freq=" << freq << " gain=" << gain << "\n";
    expect (freq > 200.0f && freq < 20000.0f, "drag sets frequency");
    expect (gain > -18.0f && gain < 18.0f, "drag sets gain");
    expect (face->controlAt (nodeX, nodeY) == 2, "node hit");
    expect (face->controlAt (24.0f + 122.0f + 56.0f, 566.0f) == 101, "type button hit");
    const float before = processor.apvts.getRawParameterValue ("b2_type")->load();
    face->cycleType (1);
    const float after = processor.apvts.getRawParameterValue ("b2_type")->load();
    std::cout << "PROBE case=cycle_type before=" << before << " after=" << after << "\n";
    expect (std::abs (after - std::fmod (before + 1.0f, 5.0f)) < 0.01f, "type button cycles");
    for (int mode = 0; mode < 3; ++mode)
    {
        const auto centre = eqparams::phaseButton (mode).getCentre();
        expect (face->controlAt (centre.x, centre.y) == 400 + mode, eqparams::phaseNames[mode]);
        flat (processor);
        setParam (processor, "b3_freq", 1000.0f);
        setParam (processor, "b3_q", 1.0f);
        setParam (processor, "b3_gain", 6.0f);
        face->selectPhase (mode);
        const double level = levelDb (processor, 1000.0f);
        const int latency = processor.getLatencySamples();
        std::cout << "PROBE case=gui_phase mode=" << eqparams::phaseNames[mode]
                  << " level_db=" << level << " latency=" << latency << "\n";
        expect (std::abs (level - 6.0) < 1.5, "phase mode level");
        expect (latency == (mode == 1 ? 512 : 0), "phase mode latency");
    }
    face->setSize (1274, 832);
    expect (face->getWidth() == 1274, "resized width");
    expect (face->controlAt (202.0f * 1.3f, 566.0f * 1.3f) == 101, "type button hit when resized");
    face->setSize (980, 640);
    for (int i = 0; i < eqparams::count; ++i)
        expect (processor.apvts.getParameter (eqparams::ids[i]) != nullptr, eqparams::ids[i]);
    if (argc > 1 && std::string (argv[1]) != "--cpu")
    {
        const auto image = editor->createComponentSnapshot (editor->getLocalBounds(), true);
        juce::File file (argv[1]);
        file.deleteFile();
        juce::FileOutputStream stream (file);
        expect (stream.openedOk(), "snapshot stream");
        juce::PNGImageFormat png;
        expect (png.writeImageToStream (image, stream), "snapshot write");
        std::cout << "GUI juce snapshot=" << file.getFullPathName() << " " << image.getWidth() << "x" << image.getHeight() << "\n";
        face->setSize (1274, 832);
        const auto large = editor->createComponentSnapshot (editor->getLocalBounds(), true);
        auto largeFile = file.getSiblingFile (file.getFileNameWithoutExtension() + "-large.png");
        largeFile.deleteFile();
        juce::FileOutputStream largeStream (largeFile);
        expect (largeStream.openedOk(), "large snapshot stream");
        expect (png.writeImageToStream (large, largeStream), "large snapshot write");
        std::cout << "GUI juce snapshot=" << largeFile.getFullPathName() << " " << large.getWidth() << "x" << large.getHeight() << "\n";
    }
}
double cpuNow()
{
    timespec ts {};
    clock_gettime (CLOCK_PROCESS_CPUTIME_ID, &ts);
    return (double) ts.tv_sec + (double) ts.tv_nsec * 1.0e-9;
}

void measureSong()
{
    constexpr int instances = 100;
    constexpr int warm = 187;
    constexpr int measure = 750;
    std::vector<float> source ((size_t) (warm + measure) * kBlock);
    float osc = 0.0f;
    const float step = 1000.0f / (float) kSampleRate;
    for (auto& sample : source)
    {
        osc = std::fmod (osc + step, 1.0f);
        sample = std::sin (osc * juce::MathConstants<float>::twoPi) * 0.25f;
    }
    auto shape = [] (EqAudioProcessor& processor) {
        for (int band = 0; band < eqparams::bands; ++band)
        {
            setParam (processor, eqparams::ids[eqparams::typeIndex (band)], 0.0f);
            setParam (processor, eqparams::ids[eqparams::gainIndex (band)], 0.0f);
        }
        setParam (processor, "trim", 0.0f);
        setParam (processor, "b1_type", 1.0f);
        setParam (processor, "b1_freq", 100.0f);
        setParam (processor, "b2_type", 2.0f);
        setParam (processor, "b2_freq", 8000.0f);
        for (int band = 2; band < 6; ++band)
        {
            setParam (processor, eqparams::ids[eqparams::gainIndex (band)], 6.0f);
            setParam (processor, eqparams::ids[eqparams::qIndex (band)], 2.0f);
        }
    };
    const struct { const char* dsp; float mode; } cases[] = { { "mp", 0.0f }, { "lp", 1.0f }, { "zl", 2.0f } };
    for (const auto& item : cases)
    {
        std::vector<double> samples;
        int latency = 0;
        for (int pass = 0; pass < 4; ++pass)
        {
            std::vector<std::unique_ptr<EqAudioProcessor>> eqs;
            eqs.reserve (instances);
            for (int n = 0; n < instances; ++n)
            {
                auto eq = std::make_unique<EqAudioProcessor>();
                eq->prepareToPlay (kSampleRate, kBlock);
                eq->setSpectrumEnabled (false);
                shape (*eq);
                setParam (*eq, "phase", item.mode);
                eqs.push_back (std::move (eq));
            }
            juce::AudioBuffer<float> block (2, kBlock);
            juce::MidiBuffer midi;
            for (int n = 0; n < warm; ++n)
            {
                for (auto& eq : eqs)
                {
                    for (int i = 0; i < kBlock; ++i)
                    {
                        const float s = source[(size_t) (n * kBlock + i)];
                        block.setSample (0, i, s);
                        block.setSample (1, i, s);
                    }
                    eq->processBlock (block, midi);
                }
            }
            latency = eqs.front()->getLatencySamples();
            float sink = 0.0f;
            const double begin = cpuNow();
            for (int n = 0; n < measure; ++n)
            {
                for (auto& eq : eqs)
                {
                    for (int i = 0; i < kBlock; ++i)
                    {
                        const float s = source[(size_t) ((warm + n) * kBlock + i)];
                        block.setSample (0, i, s);
                        block.setSample (1, i, s);
                    }
                    eq->processBlock (block, midi);
                    sink += block.getSample (0, 0);
                }
            }
            const double withCopy = cpuNow() - begin;
            const double copyBegin = cpuNow();
            for (int n = 0; n < measure; ++n)
            {
                for (int instance = 0; instance < instances; ++instance)
                {
                    for (int i = 0; i < kBlock; ++i)
                    {
                        const float s = source[(size_t) ((warm + n) * kBlock + i)];
                        block.setSample (0, i, s);
                        block.setSample (1, i, s);
                    }
                    sink += block.getSample (0, 0);
                }
            }
            const double copyOnly = cpuNow() - copyBegin;
            const double audio = (double) measure * kBlock / kSampleRate;
            const double pct = std::max (0.0, withCopy - copyOnly) / audio * 100.0;
            juce::ignoreUnused (sink);
            if (pass > 0)
                samples.push_back (pct);
        }
        std::sort (samples.begin(), samples.end());
        std::cout << "CPU plugin=juce instances=" << instances << " dsp=" << item.dsp
                  << " load=full fft=off median_pct_realtime=" << samples[samples.size() / 2]
                  << " latency=" << latency << "\n";
        std::cout.flush();
    }
}

void measureCpu()
{
    constexpr int warm = 187;
    constexpr int measure = 750;
    std::vector<float> source ((size_t) (warm + measure) * kBlock);
    float phase = 0.0f;
    const float step = 1000.0f / (float) kSampleRate;
    for (auto& sample : source)
    {
        phase = std::fmod (phase + step, 1.0f);
        sample = std::sin (phase * juce::MathConstants<float>::twoPi) * 0.25f;
    }
    auto once = [&] (const char* name, auto setup, int* latencyOut) {
        EqAudioProcessor processor;
        processor.prepareToPlay (kSampleRate, kBlock);
        processor.setSpectrumEnabled (false);
        setup (processor);
        juce::AudioBuffer<float> block (2, kBlock);
        juce::MidiBuffer midi;
        for (int n = 0; n < warm; ++n)
        {
            for (int i = 0; i < kBlock; ++i)
            {
                const float s = source[(size_t) (n * kBlock + i)];
                block.setSample (0, i, s);
                block.setSample (1, i, s);
            }
            processor.processBlock (block, midi);
        }
        if (latencyOut != nullptr)
            *latencyOut = processor.getLatencySamples();
        float sink = 0.0f;
        const double begin = cpuNow();
        for (int n = 0; n < measure; ++n)
        {
            for (int i = 0; i < kBlock; ++i)
            {
                const float s = source[(size_t) ((warm + n) * kBlock + i)];
                block.setSample (0, i, s);
                block.setSample (1, i, s);
            }
            processor.processBlock (block, midi);
            sink += block.getSample (0, 0);
        }
        const double withCopy = cpuNow() - begin;
        const double copyBegin = cpuNow();
        for (int n = 0; n < measure; ++n)
        {
            for (int i = 0; i < kBlock; ++i)
            {
                const float s = source[(size_t) ((warm + n) * kBlock + i)];
                block.setSample (0, i, s);
                block.setSample (1, i, s);
            }
            sink += block.getSample (0, 0);
        }
        const double copyOnly = cpuNow() - copyBegin;
        const double audio = (double) measure * kBlock / kSampleRate;
        const double cpu = std::max (0.0, withCopy - copyOnly);
        const double pct = cpu / audio * 100.0;
        std::cout << "CPU plugin=juce case=" << name
                  << " audio_sec=" << audio
                  << " cpu_sec=" << cpu
                  << " pct_realtime=" << pct
                  << " sink=" << sink << "\n";
        return pct;
    };
    auto unity = [] (EqAudioProcessor& processor) {
        for (int band = 0; band < eqparams::bands; ++band)
        {
            setParam (processor, eqparams::ids[eqparams::typeIndex (band)], 0.0f);
            setParam (processor, eqparams::ids[eqparams::gainIndex (band)], 0.0f);
        }
        setParam (processor, "trim", 0.0f);
    };
    auto one = [&] (EqAudioProcessor& processor) {
        unity (processor);
        setParam (processor, "b3_freq", 1000.0f);
        setParam (processor, "b3_q", 1.0f);
        setParam (processor, "b3_gain", 6.0f);
    };
    auto full = [&] (EqAudioProcessor& processor) {
        unity (processor);
        setParam (processor, "b1_type", 1.0f);
        setParam (processor, "b1_freq", 100.0f);
        setParam (processor, "b2_type", 2.0f);
        setParam (processor, "b2_freq", 8000.0f);
        for (int band = 2; band < 6; ++band)
        {
            setParam (processor, eqparams::ids[eqparams::gainIndex (band)], 6.0f);
            setParam (processor, eqparams::ids[eqparams::qIndex (band)], 2.0f);
        }
    };
    struct Case { const char* dsp; const char* load; float mode; };
    const Case cases[] = {
        { "mp", "idle", 0.0f }, { "mp", "one", 0.0f }, { "mp", "full", 0.0f },
        { "lp", "idle", 1.0f }, { "lp", "one", 1.0f }, { "lp", "full", 1.0f },
        { "zl", "idle", 2.0f }, { "zl", "one", 2.0f }, { "zl", "full", 2.0f },
    };
    for (const auto& item : cases)
    {
        std::vector<double> samples;
        int latency = 0;
        const std::string name = std::string (item.dsp) + "_" + item.load;
        for (int pass = 0; pass < 6; ++pass)
        {
            const double pct = once (name.c_str(), [&] (EqAudioProcessor& processor) {
                if (std::string (item.load) == "one") one (processor);
                else if (std::string (item.load) == "full") full (processor);
                else unity (processor);
                setParam (processor, "phase", item.mode);
            }, &latency);
            if (pass > 0)
                samples.push_back (pct);
        }
        std::sort (samples.begin(), samples.end());
        std::cout << "CPU plugin=juce dsp=" << item.dsp << " load=" << item.load
                  << " fft=off median_pct_realtime=" << samples[samples.size() / 2]
                  << " latency=" << latency << "\n";
    }
}
} // namespace

int main (int argc, char** argv)
{
    checkTypes();
    checkPhase();
    if (argc > 1 && std::string (argv[1]) == "--cpu")
    {
        if (argc > 2 && std::string (argv[2]) == "--song")
            measureSong();
        else
            measureCpu();
        return 0;
    }
    editorTree (argc, argv);
    std::cout << "PROBE juce ok\n";
    return 0;
}
