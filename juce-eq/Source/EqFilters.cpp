#include "EqFilters.h"

#include <cmath>
#include <complex>

namespace
{
void fft1024 (float* re, float* im, bool inverse)
{
    constexpr int n = 1024;
    int j = 0;
    for (int i = 1; i < n; ++i)
    {
        int bit = n >> 1;
        while ((j & bit) != 0) { j ^= bit; bit >>= 1; }
        j ^= bit;
        if (i < j) { std::swap (re[i], re[j]); std::swap (im[i], im[j]); }
    }
    for (int len = 2; len <= n; len <<= 1)
    {
        const float ang = juce::MathConstants<float>::twoPi / (float) len * (inverse ? 1.0f : -1.0f);
        const float wlenRe = std::cos (ang);
        const float wlenIm = std::sin (ang);
        for (int i = 0; i < n; i += len)
        {
            float wRe = 1.0f, wIm = 0.0f;
            for (int k = 0; k < len / 2; ++k)
            {
                const float uRe = re[i + k], uIm = im[i + k];
                const float vRe = re[i + k + len / 2] * wRe - im[i + k + len / 2] * wIm;
                const float vIm = re[i + k + len / 2] * wIm + im[i + k + len / 2] * wRe;
                re[i + k] = uRe + vRe;
                im[i + k] = uIm + vIm;
                re[i + k + len / 2] = uRe - vRe;
                im[i + k + len / 2] = uIm - vIm;
                const float next = wRe * wlenRe - wIm * wlenIm;
                wIm = wRe * wlenIm + wIm * wlenRe;
                wRe = next;
            }
        }
    }
    if (inverse)
        for (int i = 0; i < n; ++i) { re[i] /= (float) n; im[i] /= (float) n; }
}

int kind (float typeValue)
{
    return juce::jlimit (0, 4, juce::roundToInt (typeValue));
}

juce::dsp::IIR::Coefficients<float>::Ptr coeffsFor (int type, double sr, float freq, float q, float gainDb)
{
    const float qq = std::max (0.2f, q);
    switch (type)
    {
        case 1: return juce::dsp::IIR::Coefficients<float>::makeHighPass (sr, freq, qq);
        case 2: return juce::dsp::IIR::Coefficients<float>::makeLowPass (sr, freq, qq);
        case 3: return juce::dsp::IIR::Coefficients<float>::makeLowShelf (sr, freq, qq, juce::Decibels::decibelsToGain (gainDb));
        case 4: return juce::dsp::IIR::Coefficients<float>::makeHighShelf (sr, freq, qq, juce::Decibels::decibelsToGain (gainDb));
        default: return juce::dsp::IIR::Coefficients<float>::makePeakFilter (sr, freq, qq, juce::Decibels::decibelsToGain (gainDb));
    }
}
} // namespace

void EqFilters::prepare (double newSampleRate, int channelCount)
{
    sampleRate = newSampleRate;
    channels.clear();
    channels.resize ((size_t) juce::jlimit (1, maxChannels, channelCount));
    convWork.resize ((size_t) convFft * 2);
    ready = false;
    reset();
}

void EqFilters::reset()
{
    for (auto& channel : channels)
    {
        for (auto& band : channel.bands)
            band.reset();
        channel.convPos = 0;
        channel.convWritten = 0;
        std::fill (std::begin (channel.convHist), std::end (channel.convHist), 0.0f);
    }
    meter[0] = meter[1] = -80.0f;
}

void EqFilters::setParams (const float newValues[eqparams::count])
{
    bool changed = ! ready;
    for (int i = 0; i < eqparams::count; ++i)
        if (values[i] != newValues[i])
            changed = true;
    if (! changed)
        return;
    for (int i = 0; i < eqparams::count; ++i)
        values[i] = newValues[i];
    rebuild();
}

void EqFilters::rebuild()
{
    phaseMode = juce::jlimit (0, 2, juce::roundToInt (values[eqparams::phaseIndex]));
    trimLin = juce::Decibels::decibelsToGain (values[eqparams::trimIndex]);
    for (auto& channel : channels)
        for (int band = 0; band < eqparams::bands; ++band)
        {
            const int type = kind (values[eqparams::typeIndex (band)]);
            const float gain = (type == 1 || type == 2) ? 0.0f : values[eqparams::gainIndex (band)];
            channel.bands[band].coefficients = coeffsFor (type, sampleRate,
                                                          values[eqparams::freqIndex (band)],
                                                          values[eqparams::qIndex (band)],
                                                          gain);
        }
    if (phaseMode == 1)
        designLinear();
    else if (phaseMode == 2)
        designAnalogZl();
    prepareKernel();
    ready = true;
}

float EqFilters::processSample (int channel, float x)
{
    processChannel (channel, &x, 1);
    return x;
}

void EqFilters::processChannel (int channel, float* samples, int numSamples)
{
    if (samples == nullptr || numSamples <= 0 || channel < 0 || channel >= (int) channels.size())
        return;
    if (phaseMode != 0)
    {
        int offset = 0;
        while (offset < numSamples)
        {
            const int count = juce::jmin (1024, numSamples - offset);
            firChunk (channel, samples + offset, count);
            offset += count;
        }
        return;
    }
    bool active[eqparams::bands];
    for (int band = 0; band < eqparams::bands; ++band)
        active[band] = bandIsRunning (band);
    auto& state = channels[(size_t) channel];
    float peak = 0.0f;
    for (int i = 0; i < numSamples; ++i)
    {
        float y = samples[i];
        for (int band = 0; band < eqparams::bands; ++band)
            if (active[band])
                y = state.bands[band].processSample (y);
        y *= trimLin;
        peak = juce::jmax (peak, std::abs (y));
        samples[i] = y;
    }
    pushMeter (channel, peak);
}

bool EqFilters::bandIsRunning (int band) const
{
    const int type = kind (values[eqparams::typeIndex (band)]);
    return ! (type == 0 && std::abs (values[eqparams::gainIndex (band)]) < 1.0e-3f);
}

void EqFilters::pushMeter (int channel, float peak)
{
    if (channel < 0 || channel >= 2)
        return;
    const float db = peak < 1.0e-8f ? -80.0f : 20.0f * std::log10 (peak);
    meter[channel] = db > meter[channel] ? db : meter[channel] * 0.9995f + db * 0.0005f;
}

void EqFilters::prepareKernel()
{
    if ((int) convWork.size() < convFft * 2)
        convWork.resize ((size_t) convFft * 2);
    std::fill (convWork.begin(), convWork.end(), 0.0f);
    for (int i = 0; i < firLen; ++i)
        convWork[(size_t) i] = fir[i];
    fft2048.performRealOnlyForwardTransform (convWork.data(), false);
    std::copy (convWork.begin(), convWork.begin() + convFft * 2, kernel);
}

void EqFilters::firChunk (int channel, float* samples, int count)
{
    auto& state = channels[(size_t) channel];
    for (int i = 0; i < count; ++i)
    {
        state.convHist[state.convPos] = samples[i];
        state.convPos = (state.convPos + 1) % convFft;
        if (state.convWritten < 0xffffffffffffffffULL)
            ++state.convWritten;
    }
    if ((int) convWork.size() < convFft * 2)
        convWork.resize ((size_t) convFft * 2);
    std::fill (convWork.begin(), convWork.end(), 0.0f);
    for (int index = 0; index < convFft; ++index)
    {
        const int age = convFft - 1 - index;
        if ((juce::uint64) age < state.convWritten)
        {
            const int slot = (state.convPos + convFft - 1 - age) % convFft;
            convWork[(size_t) index] = state.convHist[slot];
        }
    }
    fft2048.performRealOnlyForwardTransform (convWork.data(), false);
    for (int index = 0; index < convFft; ++index)
    {
        const float xr = convWork[(size_t) index * 2];
        const float xi = convWork[(size_t) index * 2 + 1];
        const float hr = kernel[(size_t) index * 2];
        const float hi = kernel[(size_t) index * 2 + 1];
        convWork[(size_t) index * 2] = xr * hr - xi * hi;
        convWork[(size_t) index * 2 + 1] = xr * hi + xi * hr;
    }
    fft2048.performRealOnlyInverseTransform (convWork.data());
    const int start = convFft - count;
    float peak = 0.0f;
    for (int i = 0; i < count; ++i)
    {
        const float y = convWork[(size_t) (start + i)] * trimLin;
        peak = juce::jmax (peak, std::abs (y));
        samples[i] = y;
    }
    pushMeter (channel, peak);
}

float EqFilters::outputDb (int channel) const
{
    return channel >= 0 && channel < 2 ? meter[channel] : -80.0f;
}

float EqFilters::magnitudeDb (float hz) const
{
    double re = 1.0, im = 0.0;
    const double w = juce::MathConstants<double>::twoPi * hz / sampleRate;
    const double z1Re = std::cos (w), z1Im = -std::sin (w);
    const double z2Re = z1Re * z1Re - z1Im * z1Im, z2Im = 2.0 * z1Re * z1Im;
    if (channels.empty())
        return values[eqparams::trimIndex];
    for (const auto& band : channels.front().bands)
    {
        const auto c = band.coefficients;
        if (c == nullptr)
            continue;
        const double b0 = c->coefficients[0], b1 = c->coefficients[1], b2 = c->coefficients[2];
        const double a1 = c->coefficients[3], a2 = c->coefficients[4];
        const double nRe = b0 + b1 * z1Re + b2 * z2Re;
        const double nIm = b1 * z1Im + b2 * z2Im;
        const double dRe = 1.0 + a1 * z1Re + a2 * z2Re;
        const double dIm = a1 * z1Im + a2 * z2Im;
        const double den = dRe * dRe + dIm * dIm;
        if (den < 1.0e-20)
            continue;
        const double sRe = (nRe * dRe + nIm * dIm) / den;
        const double sIm = (nIm * dRe - nRe * dIm) / den;
        const double nr = re * sRe - im * sIm;
        im = re * sIm + im * sRe;
        re = nr;
    }
    return (float) (20.0 * std::log10 (std::max (1.0e-8, std::sqrt (re * re + im * im))) + values[eqparams::trimIndex]);
}

void EqFilters::designLinear()
{
    if (channels.empty())
        return;
    juce::dsp::IIR::Filter<float> scratch[eqparams::bands];
    for (int band = 0; band < eqparams::bands; ++band)
        scratch[band].coefficients = channels.front().bands[band].coefficients;
    float re[firLen], im[firLen];
    float x = 1.0f;
    for (int i = 0; i < firLen; ++i)
    {
        float y = x;
        for (auto& band : scratch)
            y = band.processSample (y);
        re[i] = y;
        im[i] = 0.0f;
        x = 0.0f;
    }
    fft1024 (re, im, false);
    for (int i = 0; i < firLen; ++i)
    {
        const float mag = std::sqrt (re[i] * re[i] + im[i] * im[i]);
        re[i] = mag;
        im[i] = 0.0f;
    }
    fft1024 (re, im, true);
    for (int i = 0; i < firLen; ++i)
        fir[(i + firLen / 2) % firLen] = re[i];
}

void EqFilters::designAnalogZl()
{
    struct Stage { float b0, b1, b2, a1, a2; };
    const double rate = sampleRate * 16.0;
    std::vector<Stage> stages;
    auto add = [&] (int type, float freq, float q, float gain)
    {
        auto c = coeffsFor (type, rate, freq, q, gain);
        stages.push_back ({ c->coefficients[0], c->coefficients[1], c->coefficients[2], c->coefficients[3], c->coefficients[4] });
    };
    for (int band = 0; band < eqparams::bands; ++band)
    {
        const int type = kind (values[eqparams::typeIndex (band)]);
        const float gain = (type == 1 || type == 2) ? 0.0f : values[eqparams::gainIndex (band)];
        add (type, values[eqparams::freqIndex (band)], values[eqparams::qIndex (band)], gain);
    }
    float re[firLen], im[firLen];
    for (int bin = 0; bin <= firLen / 2; ++bin)
    {
        const float freq = bin == 0 ? 1.0f : (float) bin * (float) sampleRate / (float) firLen;
        const double w = juce::MathConstants<double>::twoPi * freq / rate;
        const double z1Re = std::cos (w), z1Im = -std::sin (w);
        const double z2Re = z1Re * z1Re - z1Im * z1Im, z2Im = 2.0 * z1Re * z1Im;
        double hr = 1.0, hi = 0.0;
        for (const auto& s : stages)
        {
            const double nRe = s.b0 + s.b1 * z1Re + s.b2 * z2Re;
            const double nIm = s.b1 * z1Im + s.b2 * z2Im;
            const double dRe = 1.0 + s.a1 * z1Re + s.a2 * z2Re;
            const double dIm = s.a1 * z1Im + s.a2 * z2Im;
            const double den = dRe * dRe + dIm * dIm;
            const double sRe = den < 1.0e-20 ? 1.0 : (nRe * dRe + nIm * dIm) / den;
            const double sIm = den < 1.0e-20 ? 0.0 : (nIm * dRe - nRe * dIm) / den;
            const double nr = hr * sRe - hi * sIm;
            hi = hr * sIm + hi * sRe;
            hr = nr;
        }
        re[bin] = (float) hr;
        im[bin] = (float) hi;
        if (bin > 0 && bin < firLen / 2)
        {
            re[firLen - bin] = (float) hr;
            im[firLen - bin] = (float) -hi;
        }
    }
    fft1024 (re, im, true);
    for (int i = 0; i < firLen / 2; ++i)
    {
        const float fade = i > firLen / 2 - 64 ? (float) (firLen / 2 - 1 - i) / 64.0f : 1.0f;
        fir[i] = re[i] * fade;
    }
    for (int i = firLen / 2; i < firLen; ++i)
        fir[i] = 0.0f;
}
