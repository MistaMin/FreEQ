#include "PluginEditor.h"
#include "EqParams.h"

namespace
{
const juce::Colour bandColour[6] = {
    juce::Colour (0xffe88c7a), juce::Colour (0xffedcc48), juce::Colour (0xff48a860),
    juce::Colour (0xff4894d2), juce::Colour (0xff945cb8), juce::Colour (0xffd25c96)
};
const juce::Colour paper (0xffe8e4d8);
const juce::Colour ink (0xff38383c);

float shown (float value)
{
    return std::abs (value) < 5.0e-4f ? 0.0f : value;
}
constexpr int kColumns = 160;
constexpr float kFloorDb = -120.0f;
const uint32_t kBandColour[7] = { 0xffff3b4a, 0xffff9a1f, 0xffffe23a, 0xff5be85a, 0xff35e0d0, 0xff3f8ff0, 0xffc05cff };

juce::Colour colourAt (float freq)
{
    const float centres[7] = {
        std::sqrt (20.0f * 60.0f), std::sqrt (60.0f * 250.0f), std::sqrt (250.0f * 500.0f),
        std::sqrt (500.0f * 2000.0f), std::sqrt (2000.0f * 4000.0f), std::sqrt (4000.0f * 8000.0f),
        std::sqrt (8000.0f * 30000.0f)
    };
    if (freq <= centres[0]) return juce::Colour (kBandColour[0]);
    if (freq >= centres[6]) return juce::Colour (kBandColour[6]);
    int i = 0;
    while (freq > centres[i + 1]) ++i;
    const float t = (std::log (freq) - std::log (centres[i])) / (std::log (centres[i + 1]) - std::log (centres[i]));
    const auto mix = [t] (uint32_t a, uint32_t b, int shift)
    {
        const float av = (float) ((a >> shift) & 255);
        const float bv = (float) ((b >> shift) & 255);
        return (uint8_t) std::lround (av * (1.0f - t) + bv * t);
    };
    return juce::Colour (mix (kBandColour[i], kBandColour[i + 1], 16),
                         mix (kBandColour[i], kBandColour[i + 1], 8),
                         mix (kBandColour[i], kBandColour[i + 1], 0));
}
}

struct EqAudioProcessorEditor::Fft
{
    juce::dsp::FFT transform { 12 };
    std::vector<float> window, display, tilt, columns, work, frame;
    float norm = 1.0f;
    double sampleRate = 0.0;
    uint64_t lastPos = 0;
    bool haveFrame = false;
    int frames = 0;

    Fft()
    {
        window.assign ((size_t) SpectrumRing::fftSize, 0.0f);
        display.assign ((size_t) SpectrumRing::fftSize / 2 + 1, kFloorDb);
        tilt.assign (display.size(), 0.0f);
        columns.assign ((size_t) kColumns, kFloorDb);
        work.assign ((size_t) SpectrumRing::fftSize * 2, 0.0f);
        frame.assign ((size_t) SpectrumRing::fftSize, 0.0f);
    }

    void prepare (double newRate)
    {
        if (newRate < 1000.0 || std::abs (newRate - sampleRate) < 0.5)
            return;
        sampleRate = newRate;
        double sum = 0.0;
        for (int i = 0; i < SpectrumRing::fftSize; ++i)
        {
            const float w = 0.5f - 0.5f * std::cos (juce::MathConstants<float>::twoPi * (float) i / (float) SpectrumRing::fftSize);
            window[(size_t) i] = w;
            sum += w;
        }
        norm = (float) (2.0 / sum);
        for (size_t k = 0; k < tilt.size(); ++k)
        {
            if (k == 0)
                tilt[k] = 0.0f;
            else
            {
                const float hz = (float) (k * sampleRate / (double) SpectrumRing::fftSize);
                tilt[k] = 3.0f * std::log2 (hz / 1000.0f);
            }
        }
        std::fill (display.begin(), display.end(), kFloorDb);
        haveFrame = false;
    }

    void analyse (const float* samples)
    {
        std::fill (work.begin(), work.end(), 0.0f);
        for (int i = 0; i < SpectrumRing::fftSize; ++i)
            work[(size_t) i] = samples[i] * window[(size_t) i];
        transform.performFrequencyOnlyForwardTransform (work.data(), true);
        const int bins = (int) display.size();
        for (int k = 0; k < bins; ++k)
        {
            float mag = work[(size_t) k] * norm;
            if (k == 0 || k == SpectrumRing::fftSize / 2)
                mag *= 0.5f;
            const float db = mag > 1.0e-7f ? 20.0f * std::log10 (mag) : kFloorDb;
            display[(size_t) k] = std::max (display[(size_t) k], std::max (kFloorDb, db + tilt[(size_t) k]));
        }
        ++frames;
    }

    void decay (float seconds)
    {
        const float drop = 24.0f * seconds;
        for (auto& bin : display)
            bin = std::max (kFloorDb, bin - drop);
    }

    void reduce()
    {
        const int bins = (int) display.size();
        const double binHz = sampleRate / (double) SpectrumRing::fftSize;
        const double nyquist = sampleRate * 0.5;
        const double ratio = std::pow (20000.0 / 20.0, 1.0 / (double) kColumns);
        double f0 = 20.0;
        for (int column = 0; column < kColumns; ++column)
        {
            const double f1 = f0 * ratio;
            if (f0 >= nyquist)
            {
                columns[(size_t) column] = kFloorDb;
                f0 = f1;
                continue;
            }
            const double b0 = f0 / binHz;
            const double b1 = std::min (f1, nyquist) / binHz;
            const int i0 = (int) std::ceil (b0);
            const int i1 = (int) std::floor (b1);
            if (i1 >= i0)
            {
                float peak = kFloorDb;
                const int lo = std::max (1, i0);
                const int hi = std::min (bins - 1, i1);
                for (int i = lo; i <= hi; ++i)
                    peak = std::max (peak, display[(size_t) i]);
                columns[(size_t) column] = peak;
            }
            else
            {
                const double at = 0.5 * (b0 + b1);
                const int lo = std::max (0, (int) std::floor (at));
                const int hi = std::min (bins - 1, lo + 1);
                const float t = (float) (at - lo);
                columns[(size_t) column] = display[(size_t) lo] + (display[(size_t) hi] - display[(size_t) lo]) * t;
            }
            f0 = f1;
        }
    }

    bool update (const SpectrumRing& ring, double rate, float seconds)
    {
        prepare (rate);
        const auto pos = ring.position();
        if (! haveFrame || pos - lastPos >= 128)
        {
            if (ring.copyLatest (frame.data(), SpectrumRing::fftSize))
            {
                lastPos = pos;
                haveFrame = true;
                analyse (frame.data());
            }
        }
        decay (seconds);
        reduce();
        return frames > 0;
    }

    float peak() const
    {
        return *std::max_element (display.begin(), display.end());
    }
};

EqAudioProcessorEditor::EqAudioProcessorEditor (EqAudioProcessor& owner)
    : AudioProcessorEditor (owner), eq (owner), fft (std::make_unique<Fft>())
{
    setResizable (true, true);
    setResizeLimits (735, 480, 1960, 1280);
    getConstrainer()->setFixedAspectRatio (980.0 / 640.0);
    setName ("FreEQ");
    setSize (980, 640);
    startTimerHz (30);
}

float EqAudioProcessorEditor::uiScale() const
{
    return getWidth() > 0 ? (float) getWidth() / 980.0f : 1.0f;
}

juce::Point<float> EqAudioProcessorEditor::designPoint (juce::Point<float> p) const
{
    return p / uiScale();
}

EqAudioProcessorEditor::~EqAudioProcessorEditor() = default;

void EqAudioProcessorEditor::timerCallback()
{
    refreshSpectrum();
    repaint();
}

void EqAudioProcessorEditor::refreshSpectrum()
{
    const double now = juce::Time::getMillisecondCounterHiRes();
    const float dt = fftClockMs <= 0.0 ? (1.0f / 30.0f) : juce::jlimit (0.0f, 0.1f, (float) ((now - fftClockMs) * 0.001));
    fftClockMs = now;
    const double rate = eq.getSampleRate() > 1000.0 ? eq.getSampleRate() : 48000.0;
    fft->update (eq.spectrumRing(), rate, dt);
}

float EqAudioProcessorEditor::spectrumPeakDb() const
{
    return fft->peak();
}

void EqAudioProcessorEditor::paintSpectrum (juce::Graphics& g, juce::Rectangle<float> graph)
{
    const float step = graph.getWidth() / (float) kColumns;
    for (int i = 0; i < kColumns; ++i)
    {
        const float shown = juce::jlimit (0.0f, 1.0f, (fft->columns[(size_t) i] + 90.0f) / 90.0f);
        const float h = graph.getHeight() * shown;
        if (h < 1.0f)
            continue;
        const float freq = 20.0f * std::pow (1000.0f, ((float) i + 0.5f) / (float) kColumns);
        const float x = graph.getX() + (float) i * step;
        const float y = graph.getBottom() - h;
        g.setColour (colourAt (freq).withAlpha (0.55f));
        g.fillRect (x, y, step + 0.5f, h);
        g.setColour (colourAt (freq).withAlpha (0.9f));
        g.fillRect (x, y, step + 0.5f, 1.6f);
    }
}

juce::Rectangle<float> EqAudioProcessorEditor::plotBounds() const
{
    return { 24.0f, 52.0f, 730.0f, 360.0f };
}

juce::Rectangle<float> EqAudioProcessorEditor::graphBounds() const
{
    auto inner = plotBounds().reduced (14.0f);
    return inner.withTrimmedLeft (28.0f).withTrimmedRight (28.0f).withTrimmedTop (16.0f).withTrimmedBottom (20.0f);
}

float EqAudioProcessorEditor::freqToX (float freq) const
{
    const auto g = graphBounds();
    const float t = std::log (juce::jmax (20.0f, freq) / 20.0f) / std::log (1000.0f);
    return g.getX() + juce::jlimit (0.0f, 1.0f, t) * g.getWidth();
}

float EqAudioProcessorEditor::xToFreq (float x) const
{
    const auto g = graphBounds();
    const float t = juce::jlimit (0.0f, 1.0f, (x - g.getX()) / g.getWidth());
    return 20.0f * std::pow (1000.0f, t);
}

float EqAudioProcessorEditor::dbToY (float db) const
{
    const auto g = graphBounds();
    const float t = (18.0f - juce::jlimit (-18.0f, 18.0f, db)) / 36.0f;
    return g.getY() + t * g.getHeight();
}

float EqAudioProcessorEditor::yToDb (float y) const
{
    const auto g = graphBounds();
    const float t = juce::jlimit (0.0f, 1.0f, (y - g.getY()) / g.getHeight());
    return 18.0f - t * 36.0f;
}

void EqAudioProcessorEditor::setRaw (const char* id, float value)
{
    if (auto* parameter = dynamic_cast<juce::RangedAudioParameter*> (eq.apvts.getParameter (id)))
        parameter->setValueNotifyingHost (parameter->convertTo0to1 (value));
}

float EqAudioProcessorEditor::raw (const char* id) const
{
    return eq.apvts.getRawParameterValue (id)->load();
}

int EqAudioProcessorEditor::nodeAt (juce::Point<float> p) const
{
    for (int band = 5; band >= 0; --band)
    {
        const float freq = raw (eqparams::ids[eqparams::freqIndex (band)]);
        const int type = juce::roundToInt (raw (eqparams::ids[eqparams::typeIndex (band)]));
        const float db = (type == 1 || type == 2) ? eq.magnitudeDb (freq) : raw (eqparams::ids[eqparams::gainIndex (band)]);
        if (p.getDistanceFrom ({ freqToX (freq), dbToY (db) }) < 14.0f)
            return band;
    }
    return -1;
}

void EqAudioProcessorEditor::paint (juce::Graphics& g)
{
    g.fillAll (paper);
    g.addTransform (juce::AffineTransform::scale (uiScale()));
    g.setColour (juce::Colour (0xff5a8c6a));
    g.setFont (juce::Font (juce::FontOptions (26.0f, juce::Font::italic)));
    g.drawText ("FreEQ", juce::Rectangle<float> (24.0f, 12.0f, 200.0f, 32.0f), juce::Justification::centredLeft, false);

    const auto plot = plotBounds();
    g.setColour (juce::Colour (0xffc8c8c4));
    g.fillRoundedRectangle (plot, 16.0f);
    auto inner = plot.reduced (14.0f);
    g.setColour (juce::Colour (0xff142232));
    g.fillRoundedRectangle (inner, 8.0f);
    const auto graph = graphBounds();
    paintSpectrum (g, graph);

    g.setColour (juce::Colour (0x22ffffff));
    g.setFont (juce::FontOptions (11.0f));
    for (float db : { -12.0f, -6.0f, 0.0f, 6.0f, 12.0f })
    {
        const float y = dbToY (db);
        g.drawHorizontalLine ((int) y, graph.getX(), graph.getRight());
        const auto label = db > 0.0f ? "+" + juce::String ((int) db) : juce::String ((int) db);
        g.drawText (label, juce::Rectangle<float> (graph.getX() - 32.0f, y - 7.0f, 28.0f, 14.0f), juce::Justification::centredRight, false);
    }
    for (float freq : { 31.0f, 125.0f, 500.0f, 2000.0f, 8000.0f })
    {
        const float x = freqToX (freq);
        g.drawVerticalLine ((int) x, graph.getY(), graph.getBottom());
        const auto label = freq >= 1000.0f ? juce::String ((int) (freq / 1000.0f)) + "K" : juce::String ((int) freq);
        g.drawText (label, juce::Rectangle<float> (x - 18.0f, graph.getBottom() + 4.0f, 36.0f, 16.0f), juce::Justification::centred, false);
    }
    g.setColour (juce::Colours::white.withAlpha (0.8f));
    g.drawText ("View", juce::Rectangle<float> (graph.getRight() - 64.0f, graph.getY() + 4.0f, 52.0f, 16.0f), juce::Justification::centredRight, false);

    juce::Path curve;
    for (int step = 0; step <= 96; ++step)
    {
        const float t = (float) step / 96.0f;
        const float freq = 20.0f * std::pow (1000.0f, t);
        const float y = dbToY (juce::jlimit (-18.0f, 18.0f, eq.magnitudeDb (freq)));
        const float x = graph.getX() + t * graph.getWidth();
        if (step == 0) curve.startNewSubPath (x, y);
        else curve.lineTo (x, y);
    }
    g.setColour (juce::Colour (0xffedc848));
    g.strokePath (curve, juce::PathStrokeType (2.4f));

    for (int band = 0; band < 6; ++band)
    {
        const float freq = raw (eqparams::ids[eqparams::freqIndex (band)]);
        const int type = juce::roundToInt (raw (eqparams::ids[eqparams::typeIndex (band)]));
        const float db = (type == 1 || type == 2) ? eq.magnitudeDb (freq) : raw (eqparams::ids[eqparams::gainIndex (band)]);
        const auto centre = juce::Point<float> (freqToX (freq), dbToY (db));
        g.setColour (bandColour[band]);
        g.fillEllipse (centre.x - 11.0f, centre.y - 11.0f, 22.0f, 22.0f);
        g.setColour (juce::Colours::white);
        g.setFont (juce::FontOptions (12.0f));
        g.drawText (juce::String (band + 1), juce::Rectangle<float> (centre.x - 11.0f, centre.y - 9.0f, 22.0f, 18.0f), juce::Justification::centred, false);
    }

    const float tracks[2] = { 824.0f, 888.0f };
    g.setFont (juce::FontOptions (12.0f));
    g.setColour (juce::Colour (0xff737378));
    for (int ch = 0; ch < 2; ++ch)
        g.drawText (juce::String (shown (eq.outputDb (ch)), 1), juce::Rectangle<float> (tracks[ch] - 24.0f, 68.0f, 56.0f, 16.0f), juce::Justification::centred, false);
    for (int ch = 0; ch < 2; ++ch)
    {
        g.setColour (juce::Colour (0xff8c9cc8));
        g.fillRoundedRectangle (tracks[ch], 100.0f, 8.0f, 168.0f, 3.0f);
        const float t = juce::jlimit (0.0f, 1.0f, (eq.outputDb (ch) + 60.0f) / 60.0f);
        const float cap = juce::jlimit (100.0f, 268.0f, 100.0f + 168.0f * (1.0f - t)) - 8.0f;
        g.setColour (juce::Colour (0xfff29e2e));
        g.fillEllipse (tracks[ch] - 6.0f, cap, 20.0f, 16.0f);
    }
    g.setColour (ink);
    g.setFont (juce::FontOptions (13.0f));
    g.drawText ("Output", juce::Rectangle<float> (800.0f, 276.0f, 120.0f, 18.0f), juce::Justification::centred, false);
    g.drawText ("Trim", juce::Rectangle<float> (800.0f, 348.0f, 120.0f, 18.0f), juce::Justification::centred, false);
    g.setColour (juce::Colour (0xffd0d0cc));
    g.fillRoundedRectangle (820.0f, 378.0f, 80.0f, 24.0f, 8.0f);
    g.setColour (ink);
    g.drawText (juce::String (shown (raw ("trim")), 1), juce::Rectangle<float> (820.0f, 378.0f, 80.0f, 24.0f), juce::Justification::centred, false);
    g.setColour (ink);
    g.setFont (juce::FontOptions (13.0f));
    g.drawText ("Phase", juce::Rectangle<float> (786.0f, 404.0f, 184.0f, 18.0f), juce::Justification::centred, false);
    const int phase = juce::jlimit (0, 2, juce::roundToInt (raw ("phase")));
    g.setFont (juce::FontOptions (13.0f));
    for (int mode = 0; mode < 3; ++mode)
    {
        const auto button = eqparams::phaseButton (mode);
        const bool on = mode == phase;
        g.setColour (on ? juce::Colour (0xff476156) : juce::Colour (0xfff4f4f2));
        g.fillRoundedRectangle (button, 8.0f);
        g.setColour (on ? juce::Colours::white : ink);
        g.drawText (eqparams::phaseNames[mode], button, juce::Justification::centred, false);
    }

    for (int band = 0; band < 6; ++band)
    {
        const float x = 24.0f + (float) band * 122.0f;
        const float y = 430.0f;
        g.setColour (juce::Colour (0xffdcdcd8));
        g.fillRoundedRectangle (x, y, 112.0f, 190.0f, 12.0f);
        g.setColour (bandColour[band]);
        g.fillRoundedRectangle (x + 8.0f, y + 8.0f, 96.0f, 22.0f, 8.0f);
        g.setColour (juce::Colours::white);
        g.setFont (juce::FontOptions (13.0f));
        g.drawText (juce::String (band + 1), juce::Rectangle<float> (x, y + 8.0f, 112.0f, 22.0f), juce::Justification::centred, false);
        const char* rows[3] = { "G", "F", "Q" };
        const int indexes[3] = { eqparams::gainIndex (band), eqparams::freqIndex (band), eqparams::qIndex (band) };
        g.setFont (juce::FontOptions (12.0f));
        for (int row = 0; row < 3; ++row)
        {
            const float ry = y + 40.0f + (float) row * 28.0f;
            g.setColour (ink);
            g.drawText (rows[row], juce::Rectangle<float> (x + 4.0f, ry - 2.0f, 20.0f, 20.0f), juce::Justification::centred, false);
            g.setColour (juce::Colour (0xffccccc8));
            g.fillRoundedRectangle (x + 26.0f, ry - 2.0f, 76.0f, 20.0f, 6.0f);
            g.setColour (ink);
            const float number = shown (raw (eqparams::ids[indexes[row]]));
            const auto text = row == 1 ? juce::String (juce::roundToInt (number)) : juce::String (number, row == 0 ? 1 : 2);
            g.drawText (text, juce::Rectangle<float> (x + 26.0f, ry - 2.0f, 76.0f, 20.0f), juce::Justification::centred, false);
        }
        g.setColour (juce::Colour (0xfff4f4f2));
        g.fillRoundedRectangle (x + 8.0f, y + 128.0f, 96.0f, 28.0f, 8.0f);
        g.setColour (ink);
        g.setFont (juce::FontOptions (12.0f));
        const int type = juce::jlimit (0, 4, juce::roundToInt (raw (eqparams::ids[eqparams::typeIndex (band)])));
        g.drawText (eqparams::typeNames[type], juce::Rectangle<float> (x + 8.0f, y + 128.0f, 96.0f, 28.0f), juce::Justification::centred, false);
    }
}

void EqAudioProcessorEditor::mouseDown (const juce::MouseEvent& event)
{
    const auto p = designPoint (event.position);
    dragNode = nodeAt (p);
    dragValue = -1;
    if (dragNode >= 0)
        return;
    for (int band = 0; band < 6; ++band)
    {
        const auto typeBox = juce::Rectangle<float> (24.0f + (float) band * 122.0f + 8.0f, 558.0f, 96.0f, 28.0f);
        if (typeBox.contains (p))
        {
            const auto* id = eqparams::ids[eqparams::typeIndex (band)];
            const int next = (juce::roundToInt (raw (id)) + 1) % 5;
            setRaw (id, (float) next);
            return;
        }
        for (int row = 0; row < 3; ++row)
        {
            const auto box = juce::Rectangle<float> (24.0f + (float) band * 122.0f + 26.0f, 468.0f + (float) row * 28.0f, 76.0f, 20.0f);
            if (box.contains (p))
            {
                dragValue = band * 3 + row;
                const int indexes[3] = { eqparams::gainIndex (band), eqparams::freqIndex (band), eqparams::qIndex (band) };
                dragStart = raw (eqparams::ids[indexes[row]]);
                dragOriginY = p.y;
                return;
            }
        }
    }
    if (juce::Rectangle<float> (820.0f, 378.0f, 80.0f, 24.0f).contains (p))
    {
        dragValue = 100;
        dragStart = raw ("trim");
        dragOriginY = p.y;
        return;
    }
    for (int mode = 0; mode < 3; ++mode)
        if (eqparams::phaseButton (mode).contains (p))
        {
            selectPhase (mode);
            return;
        }
}

void EqAudioProcessorEditor::selectPhase (int mode)
{
    setRaw ("phase", (float) juce::jlimit (0, 2, mode));
}

void EqAudioProcessorEditor::mouseDrag (const juce::MouseEvent& event)
{
    const auto p = designPoint (event.position);
    if (dragNode >= 0)
    {
        setRaw (eqparams::ids[eqparams::freqIndex (dragNode)], xToFreq (p.x));
        const int type = juce::roundToInt (raw (eqparams::ids[eqparams::typeIndex (dragNode)]));
        if (type != 1 && type != 2)
            setRaw (eqparams::ids[eqparams::gainIndex (dragNode)], yToDb (p.y));
        return;
    }
    if (dragValue == 100)
    {
        setRaw ("trim", juce::jlimit (-24.0f, 24.0f, dragStart + (dragOriginY - p.y) * 0.05f));
        return;
    }
    if (dragValue >= 0)
    {
        const int band = dragValue / 3;
        const int row = dragValue % 3;
        const int indexes[3] = { eqparams::gainIndex (band), eqparams::freqIndex (band), eqparams::qIndex (band) };
        const float delta = (dragOriginY - p.y) * (row == 1 ? 8.0f : 0.05f);
        float next = dragStart + delta;
        if (row == 0) next = juce::jlimit (-18.0f, 18.0f, next);
        if (row == 1) next = juce::jlimit (20.0f, 20000.0f, next);
        if (row == 2) next = juce::jlimit (0.2f, 8.0f, next);
        setRaw (eqparams::ids[indexes[row]], next);
    }
}

void EqAudioProcessorEditor::mouseUp (const juce::MouseEvent&)
{
    dragNode = -1;
    dragValue = -1;
}

int EqAudioProcessorEditor::controlAt (float x, float y) const
{
    const auto p = designPoint ({ x, y });
    if (const int node = nodeAt (p); node >= 0)
        return node;
    for (int band = 0; band < 6; ++band)
    {
        if (juce::Rectangle<float> (24.0f + (float) band * 122.0f + 8.0f, 558.0f, 96.0f, 28.0f).contains (p))
            return 100 + band;
        for (int row = 0; row < 3; ++row)
            if (juce::Rectangle<float> (24.0f + (float) band * 122.0f + 26.0f, 468.0f + (float) row * 28.0f, 76.0f, 20.0f).contains (p))
                return 200 + band * 3 + row;
    }
    if (juce::Rectangle<float> (820.0f, 378.0f, 80.0f, 24.0f).contains (p))
        return 300;
    for (int mode = 0; mode < 3; ++mode)
        if (eqparams::phaseButton (mode).contains (p))
            return 400 + mode;
    return -1;
}

void EqAudioProcessorEditor::moveNode (int band, float x, float y)
{
    setRaw (eqparams::ids[eqparams::freqIndex (band)], xToFreq (x));
    const int type = juce::roundToInt (raw (eqparams::ids[eqparams::typeIndex (band)]));
    if (type != 1 && type != 2)
        setRaw (eqparams::ids[eqparams::gainIndex (band)], yToDb (y));
}

void EqAudioProcessorEditor::cycleType (int band)
{
    const auto* id = eqparams::ids[eqparams::typeIndex (band)];
    setRaw (id, (float) ((juce::roundToInt (raw (id)) + 1) % 5));
}
