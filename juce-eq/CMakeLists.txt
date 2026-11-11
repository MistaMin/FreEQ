cmake_minimum_required(VERSION 3.22)
project(EQ_BENCHMARK VERSION 0.1.0)

# Default is the JUCE 8 security line. Pass -DEQ_JUCE_PATH=.../JUCE-9.0.5 for the JUCE 9 build.
set(EQ_JUCE_PATH "/Volumes/Developer 2/Dependencies/JUCE-8.0.4" CACHE PATH "JUCE source tree")
add_subdirectory("${EQ_JUCE_PATH}" JUCE)

juce_add_plugin(EqBenchmark
    COMPANY_NAME "MistaMin"
    IS_SYNTH FALSE
    NEEDS_MIDI_INPUT FALSE
    NEEDS_MIDI_OUTPUT FALSE
    IS_MIDI_EFFECT FALSE
    EDITOR_WANTS_KEYBOARD_FOCUS FALSE
    COPY_PLUGIN_AFTER_BUILD FALSE
    PLUGIN_MANUFACTURER_CODE Mmst
    PLUGIN_CODE Eqb1
    FORMATS Standalone
    PRODUCT_NAME "EQ Benchmark JUCE")

target_sources(EqBenchmark
    PRIVATE
        Source/PluginProcessor.cpp
        Source/PluginEditor.cpp
        Source/EqFilters.cpp
        Source/EqSpectrum.cpp)

target_compile_definitions(EqBenchmark
    PUBLIC
        JUCE_WEB_BROWSER=0
        JUCE_USE_CURL=0
        JUCE_VST3_CAN_REPLACE_VST2=0)

target_link_libraries(EqBenchmark
    PRIVATE
        juce::juce_audio_utils
        juce::juce_dsp
        juce::juce_recommended_config_flags
        juce::juce_recommended_warning_flags)

juce_add_console_app(EqProbe PRODUCT_NAME "EQ Probe")

target_sources(EqProbe
    PRIVATE
        Source/ProbeMain.cpp
        Source/PluginProcessor.cpp
        Source/PluginEditor.cpp
        Source/EqFilters.cpp
        Source/EqSpectrum.cpp)

target_compile_definitions(EqProbe
    PRIVATE
        JUCE_WEB_BROWSER=0
        JUCE_USE_CURL=0
        JUCE_VST3_CAN_REPLACE_VST2=0)

target_link_libraries(EqProbe
    PRIVATE
        juce::juce_audio_utils
        juce::juce_dsp
        juce::juce_recommended_config_flags
        juce::juce_recommended_warning_flags)
