import {
  createSignal,
  onMount,
  For,
  Show,
  onCleanup,
  createEffect,
} from "solid-js";
import {
  commands,
  events,
  type WhisperModelInfo,
  type Transcript,
} from "./bindings";
import HotkeySelector from "./components/HotkeySelector";
import Timeline from "./components/Timeline";
import TitleBar from "./components/TitleBar";
import { Toaster, toast } from "solid-sonner";

interface AudioDevice {
  id: string;
  name: string;
  is_default: boolean;
}

function App() {
  const [wordCount, setWordCount] = createSignal(0);
  const [isRecording, setIsRecording] = createSignal(false);
  const [audioDevices, setAudioDevices] = createSignal<AudioDevice[]>([]);
  const [selectedDevice, setSelectedDevice] = createSignal<string>("");
  const [currentHotkey, setCurrentHotkey] = createSignal<string | null>(null);
  const [showHotkeySelector, setShowHotkeySelector] = createSignal(false);
  const [micPermission, setMicPermission] = createSignal(false);
  const [accessibilityPermission, setAccessibilityPermission] =
    createSignal(false);
  const [fnKeyPressed, setFnKeyPressed] = createSignal(false);
  const [modelDownloaded, setModelDownloaded] = createSignal(false);
  const [isDownloading, setIsDownloading] = createSignal(false);
  const [downloadProgress, setDownloadProgress] = createSignal(0);
  const [downloadedBytes, setDownloadedBytes] = createSignal(0);
  const [totalBytes, setTotalBytes] = createSignal(0);
  const [availableModels, setAvailableModels] = createSignal<
    WhisperModelInfo[]
  >([]);
  const [downloadedModels, setDownloadedModels] = createSignal<string[]>([]);
  const [selectedModel, setSelectedModel] = createSignal<string | null>(null);
  const [systemInfo, setSystemInfo] = createSignal<{ isAppleSilicon: boolean }>(
    { isAppleSilicon: false }
  );
  const [recordingStats, setRecordingStats] = createSignal({
    total_words: 0,
    total_time_ms: 0,
    overall_wpm: 0,
    session_words: 0,
    session_time_ms: 0,
    session_wpm: 0,
  });
  const [transcripts, setTranscripts] = createSignal<Transcript[]>([]);
  const [showModelSelector, setShowModelSelector] = createSignal(false);
  const [isManualRecording, setIsManualRecording] = createSignal(false);
  const [isProcessingTranscription, setIsProcessingTranscription] =
    createSignal(false);
  const [recordingStartTime, setRecordingStartTime] = createSignal<
    number | null
  >(null);
  const [recordingDuration, setRecordingDuration] = createSignal(0);

  const modifierOptions = [
    { value: "rightOption", label: "Right Option (⌥)" },
    { value: "leftOption", label: "Left Option (⌥)" },
    { value: "leftControl", label: "Left Control (⌃)" },
    { value: "rightControl", label: "Right Control (⌃)" },
    { value: "fn", label: "Fn" },
    { value: "rightCommand", label: "Right Command (⌘)" },
    { value: "rightShift", label: "Right Shift (⇧)" },
  ];

  const loadTranscripts = async () => {
    try {
      const result = await commands.getTranscripts(null);
      if (result.status === "ok") {
        setTranscripts(result.data);
      }
    } catch (error) {
      console.error("Failed to load transcripts:", error);
    }
  };

  const loadTranscriptStats = async () => {
    try {
      const result = await commands.getTranscriptStats();
      if (result.status === "ok") {
        const stats = result.data;
        setWordCount(stats.total_words);
        setRecordingStats({
          total_words: stats.total_words,
          total_time_ms: stats.total_time_ms,
          overall_wpm: stats.overall_wpm,
          session_words: 0,
          session_time_ms: 0,
          session_wpm: 0,
        });
      }
    } catch (error) {
      console.error("Failed to load transcript stats:", error);
    }
  };

  onMount(async () => {
    const isAppleSilicon =
      navigator.userAgent.includes("Mac") &&
      (navigator.userAgent.includes("ARM") ||
        (window.navigator as any).cpuClass === "ARM" ||
        (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1));
    setSystemInfo({ isAppleSilicon });

    const modelsResult = await commands.getAvailableModels();
    if (modelsResult.status === "ok") {
      setAvailableModels(modelsResult.data);
    }

    const downloadedResult = await commands.getDownloadedModels();
    let downloadedModelsList: string[] = [];
    if (downloadedResult.status === "ok") {
      downloadedModelsList = downloadedResult.data;
      setDownloadedModels(downloadedResult.data);
    }

    const selectedResult = await commands.getSelectedModel();
    if (selectedResult.status === "ok" && selectedResult.data) {
      setSelectedModel(selectedResult.data);
      setModelDownloaded(downloadedModelsList.includes(selectedResult.data));
    } else if (modelsResult.status === "ok" && modelsResult.data.length > 0) {
      const recommended = modelsResult.data.find(
        (m) => m.id === "large-v3-turbo-q8_0"
      );
      if (recommended) {
        setSelectedModel(recommended.id);
        await commands.setSelectedModel(recommended.id);
      }
    }

    const progressUnlisten = await events.modelDownloadProgress.listen(
      (event) => {
        setDownloadProgress(event.payload.progress);
        setDownloadedBytes(event.payload.downloaded_bytes);
        setTotalBytes(event.payload.total_bytes);
      }
    );

    const completeUnlisten = await events.modelDownloadComplete.listen(
      async (event) => {
        setModelDownloaded(event.payload.success);
        setIsDownloading(false);
        if (!event.payload.success && event.payload.error) {
          console.error("Model download failed:", event.payload.error);
        } else if (event.payload.success) {
          const downloadedResult = await commands.getDownloadedModels();
          if (downloadedResult.status === "ok") {
            setDownloadedModels(downloadedResult.data);
          }
        }
      }
    );

    try {
      const devicesResult = await commands.getAudioDevices();
      if (devicesResult.status === "ok") {
        setAudioDevices(devicesResult.data);

        const currentDeviceResult = await commands.getCurrentDevice();
        if (currentDeviceResult.status === "ok" && currentDeviceResult.data) {
          setSelectedDevice(currentDeviceResult.data);
        } else {
          const defaultDevice = devicesResult.data.find((d) => d.is_default);
          if (defaultDevice) {
            setSelectedDevice(defaultDevice.id);
            await commands.setRecordingDevice(defaultDevice.id);
          }
        }
      }

      const countResult = await commands.getWordCount();
      if (countResult.status === "ok") {
        setWordCount(countResult.data);
      }

      await loadTranscripts();
      await loadTranscriptStats();

      const permissionsResult = await commands.checkPermissions();
      if (permissionsResult.status === "ok") {
        setMicPermission(permissionsResult.data.microphone.state === "Granted");
        setAccessibilityPermission(
          permissionsResult.data.accessibility.state === "Granted"
        );
      }

      const hotkeyResult = await commands.getHotkey();
      if (hotkeyResult.status === "ok" && hotkeyResult.data) {
        setCurrentHotkey(hotkeyResult.data);
        console.log("Registering saved hotkey:", hotkeyResult.data);
        const setHotkeyResult = await commands.setHotkey(hotkeyResult.data);
        if (setHotkeyResult.status === "ok") {
          console.log("Hotkey registered successfully");
        } else {
          console.error("Failed to register hotkey");
        }
      }
    } catch (error) {
      console.error("Failed to load initial data:", error);
    }

    const fnKeyUnlisten = await events.fnKeyStateChanged.listen((event) => {
      console.log("Fn key state changed:", event.payload.is_pressed);
      setFnKeyPressed(event.payload.is_pressed);
    });

    const recordingUnlisten = await events.recordingStateChanged.listen(
      (event) => {
        console.log("Recording state changed:", event.payload.is_recording);
        setIsRecording(event.payload.is_recording);
        if (!event.payload.is_recording && isManualRecording()) {
          setIsManualRecording(false);
        }
      }
    );

    const statsUnlisten = await events.recordingStatsUpdated.listen(
      async (event) => {
        console.log("Recording stats updated:", event.payload);
        setRecordingStats(event.payload);
        await loadTranscripts();
      }
    );

    const permissionCheckInterval = setInterval(refreshPermissions, 5000);

    const handleWindowFocus = () => {
      refreshPermissions();
    };

    window.addEventListener("focus", handleWindowFocus);

    onCleanup(() => {
      fnKeyUnlisten();
      recordingUnlisten();
      progressUnlisten();
      completeUnlisten();
      statsUnlisten();
      clearInterval(permissionCheckInterval);
      window.removeEventListener("focus", handleWindowFocus);
    });
  });

  const handleDeviceChange = async (deviceId: string) => {
    try {
      const result = await commands.setRecordingDevice(deviceId);
      if (result.status === "ok") {
        setSelectedDevice(deviceId);
      }
    } catch (error) {
      console.error("Failed to set recording device:", error);
    }
  };

  const requestMicrophonePermission = async () => {
    try {
      const result = await commands.requestMicrophonePermission();
      if (result.status === "ok") {
        const permissionsResult = await commands.checkPermissions();
        if (permissionsResult.status === "ok") {
          setMicPermission(
            permissionsResult.data.microphone.state === "Granted"
          );
        }
      }
    } catch (error) {
      console.error("Failed to request microphone permission:", error);
    }
  };

  const requestAccessibilityPermission = async () => {
    try {
      await commands.requestAccessibilityPermission();

      const permissionsResult = await commands.checkPermissions();
      if (permissionsResult.status === "ok") {
        setAccessibilityPermission(
          permissionsResult.data.accessibility.state === "Granted"
        );
      }
    } catch (error) {
      console.error("Failed to request accessibility permission:", error);
    }
  };

  const handleHotkeyChange = async (hotkey: string) => {
    try {
      const result = await commands.setHotkey(hotkey);
      if (result.status === "ok") {
        setCurrentHotkey(hotkey);
        setShowHotkeySelector(false);
      }
    } catch (error) {
      console.error("Failed to set hotkey:", error);
    }
  };

  const handleHotkeyCancel = () => {
    setShowHotkeySelector(false);
  };

  const refreshPermissions = async () => {
    try {
      const permissionsResult = await commands.checkPermissions();
      if (permissionsResult.status === "ok") {
        setMicPermission(permissionsResult.data.microphone.state === "Granted");
        setAccessibilityPermission(
          permissionsResult.data.accessibility.state === "Granted"
        );
      }
    } catch (error) {
      console.error("Failed to refresh permissions:", error);
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 MB";
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(1)} MB`;
  };

  const handleModelDownload = async () => {
    try {
      setIsDownloading(true);
      setDownloadProgress(0);
      setDownloadedBytes(0);
      setTotalBytes(0);
      toast.info("Starting model download...");

      const result = await commands.downloadWhisperModel();
      if (result.status === "error") {
        console.error("Failed to download model:", result.error);
        setIsDownloading(false);
        toast.error("Failed to download model");
      }
    } catch (error) {
      console.error("Failed to download model:", error);
      setIsDownloading(false);
      toast.error("Failed to download model");
    }
  };

  const handleCopyText = (text: string) => {
    toast.success("Copied to clipboard");
  };

  const handleDeleteTranscript = async (id: string) => {
    try {
      const result = await commands.deleteTranscript(id);
      if (result.status === "ok") {
        await loadTranscripts();
        await loadTranscriptStats();
        toast.success("Transcript deleted");
      } else {
        toast.error("Failed to delete transcript");
      }
    } catch (error) {
      console.error("Failed to delete transcript:", error);
      toast.error("Failed to delete transcript");
    }
  };

  const handleManualRecordingToggle = async () => {
    if (!micPermission() || !modelDownloaded()) {
      toast.error(
        "Please grant microphone permission and download a model first"
      );
      return;
    }

    if (isManualRecording()) {
      setIsManualRecording(false);
      setIsProcessingTranscription(true);

      try {
        const timeoutPromise = new Promise((_, reject) => {
          setTimeout(() => reject(new Error("Transcription timeout")), 60000);
        });

        const result = (await Promise.race([
          commands.stopRecordingManual(),
          timeoutPromise,
        ])) as Awaited<ReturnType<typeof commands.stopRecordingManual>>;

        if (result.status === "ok") {
          const transcribedText = result.data;
          if (transcribedText && transcribedText.trim() !== "") {
            await navigator.clipboard.writeText(transcribedText);
            toast.success("Transcription copied to clipboard");
          } else {
            toast.info("No speech detected");
          }
          await loadTranscripts();
          await loadTranscriptStats();
        } else if (result.status === "error") {
          console.error("Error stopping recording:", result.error);
          toast.error("Failed to transcribe audio");
        }
      } catch (error) {
        console.error("Failed to stop recording:", error);
        if (
          error instanceof Error &&
          error.message === "Transcription timeout"
        ) {
          toast.error(
            "Transcription is taking too long. Try a shorter recording."
          );
        } else {
          toast.error("Failed to stop recording");
        }
      } finally {
        setIsProcessingTranscription(false);
      }
    } else {
      try {
        const result = await commands.startRecording();
        if (result.status === "ok") {
          setIsManualRecording(true);
          setRecordingStartTime(Date.now());

          const interval = setInterval(() => {
            if (isManualRecording()) {
              const elapsed = Date.now() - (recordingStartTime() || Date.now());
              setRecordingDuration(Math.floor(elapsed / 1000));

              if (elapsed > 300000) {
                clearInterval(interval);
                toast.warning("Maximum recording duration reached (5 minutes)");
                handleManualRecordingToggle();
              }
            } else {
              clearInterval(interval);
              setRecordingDuration(0);
            }
          }, 100);
        } else {
          toast.error("Failed to start recording");
        }
      } catch (error) {
        console.error("Failed to start recording:", error);
        toast.error("Failed to start recording");
      }
    }
  };

  return (
    <div class="min-h-screen bg-dark relative overflow-hidden">
      <TitleBar />
      <Toaster
        position="top-center"
        toastOptions={{
          style: {
            background: "#2b2b2b",
            color: "#fff",
            "font-size": "14px",
            "border-color": "#3b3b3b",
          },
        }}
      />
      <div class="flex flex-col h-screen pt-10">
        <div class="px-8 py-4 border-b border-dark-secondary">
          <div class="max-w-6xl mx-auto">
            <div class="flex items-start justify-between">
              <div>
                <h1 class="text-3xl font-bold text-white mb-2 flex items-center gap-2 group cursor-pointer">
                  <svg
                    class="w-10 h-auto border border-dark-secondary rounded-[8px] transition-transform duration-300"
                    viewBox="0 0 430 430"
                    fill="none"
                    xmlns="http://www.w3.org/2000/svg"
                  >
                    <rect width="430" height="430" rx="85" fill="#1B1B1B" />
                    <rect
                      x="71"
                      y="175"
                      width="41"
                      height="163"
                      rx="20"
                      fill="white"
                    />
                    <rect
                      x="133"
                      y="92"
                      width="40"
                      height="246"
                      rx="20"
                      fill="white"
                    />
                    <rect
                      x="195"
                      y="223"
                      width="40"
                      height="115"
                      rx="20"
                      fill="white"
                    />
                    <rect
                      x="257"
                      y="175"
                      width="40"
                      height="163"
                      rx="20"
                      fill="white"
                    />
                    <rect
                      x="318"
                      y="122"
                      width="41"
                      height="216"
                      rx="20"
                      fill="white"
                    />
                  </svg>

                  <span class="overflow-hidden max-w-0 opacity-0 whitespace-nowrap transition-all duration-500 ease-out group-hover:max-w-xs group-hover:opacity-100">
                    TalkType.
                  </span>
                </h1>
                <p class="text-gray-400 text-sm">Type with your voice.</p>
              </div>
              <div class="flex gap-12">
                <div class="text-center">
                  <div class="text-4xl font-bold text-white mb-1">
                    {wordCount()}
                  </div>
                  <p class="text-gray-500 text-xs">words transcribed</p>
                </div>
                <div class="text-center">
                  <div class="text-4xl font-bold text-white mb-1">
                    {recordingStats().overall_wpm.toFixed(0)}
                  </div>
                  <p class="text-gray-500 text-xs">words per minute</p>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div class="flex flex-1 overflow-hidden">
          <div class="w-1/2 border-r border-dark-secondary overflow-hidden relative">
            <div class="p-8 pb-20">
              <h2 class="text-xl font-semibold text-white mb-6">Timeline</h2>
            </div>
            <div class="absolute top-16 left-0 right-0 bottom-0">
              <Timeline
                transcripts={transcripts()}
                onCopyText={handleCopyText}
                onDeleteTranscript={handleDeleteTranscript}
              />
            </div>

            <div class="absolute top-16 left-0 right-0 h-16 bg-gradient-to-b from-[#1b1b1b] via-[#1b1b1b]/20 to-transparent pointer-events-none z-20"></div>

            <div class="absolute bottom-0 left-0 right-0 h-28 bg-gradient-to-t from-[#1b1b1b] via-[#1b1b1b]/20 to-transparent pointer-events-none z-20"></div>

            <div class="absolute bottom-8 left-1/2 transform -translate-x-1/2 z-30">
              <button
                onClick={handleManualRecordingToggle}
                class={`relative flex items-center justify-center w-14 h-14 rounded-full transition-all duration-200 border border-white/10 hover:border-white/20 hover:scale-102 z-30 ${
                  isManualRecording()
                    ? "bg-white text-dark scale-110"
                    : isProcessingTranscription()
                    ? "bg-white text-dark"
                    : "bg-dark-secondary text-white hover:bg-dark-tertiary"
                }`}
                style={{
                  "box-shadow": "0 0 40px 0 rgba(255,255,255,0.25)",
                }}
                disabled={
                  !micPermission() ||
                  !modelDownloaded() ||
                  isProcessingTranscription()
                }
              >
                <Show
                  when={!isManualRecording() && !isProcessingTranscription()}
                  fallback={
                    <Show
                      when={isManualRecording()}
                      fallback={
                        <>
                          <div class="absolute inset-0 flex items-center justify-center">
                            <div class="relative w-8 h-8">
                              <div
                                class="absolute top-0 left-1/2 -translate-x-1/2 w-2 h-2 bg-dark rounded-full animate-spin"
                                style="transform-origin: 50% 200%"
                              ></div>
                              <div
                                class="absolute top-0 left-1/2 -translate-x-1/2 w-2 h-2 bg-dark rounded-full animate-spin"
                                style="transform-origin: 50% 200%; animation-delay: 0.2s; opacity: 0.7"
                              ></div>
                              <div
                                class="absolute top-0 left-1/2 -translate-x-1/2 w-2 h-2 bg-dark rounded-full animate-spin"
                                style="transform-origin: 50% 200%; animation-delay: 0.4s; opacity: 0.4"
                              ></div>
                            </div>
                          </div>
                        </>
                      }
                    >
                      <>
                        <div class="absolute inset-0 flex items-center justify-center">
                          <div class="flex gap-1">
                            <div class="w-1 h-3 bg-dark rounded-full animate-pulse"></div>
                            <div
                              class="w-1 h-5 bg-dark rounded-full animate-pulse"
                              style="animation-delay: 0.1s"
                            ></div>
                            <div
                              class="w-1 h-4 bg-dark rounded-full animate-pulse"
                              style="animation-delay: 0.2s"
                            ></div>
                          </div>
                        </div>
                      </>
                    </Show>
                  }
                >
                  <svg
                    class="w-6 h-6"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z"
                    />
                  </svg>
                </Show>
              </button>
              <Show when={isManualRecording()}>
                <p class="absolute top-full mt-2 left-1/2 transform -translate-x-1/2 text-xs text-gray-500 whitespace-nowrap">
                  {recordingDuration() < 60
                    ? `${recordingDuration()}s`
                    : `${Math.floor(recordingDuration() / 60)}:${(
                        recordingDuration() % 60
                      )
                        .toString()
                        .padStart(2, "0")}`}{" "}
                  - Click to stop
                </p>
              </Show>
              <Show when={isProcessingTranscription()}>
                <p class="absolute top-full mt-2 left-1/2 transform -translate-x-1/2 text-xs text-gray-500 whitespace-nowrap">
                  Processing...
                </p>
              </Show>
            </div>
          </div>

          <div class="w-1/2 p-8 overflow-y-auto">
            <div class="max-w-md">
              <h2 class="text-xl font-semibold text-white mb-6">Settings</h2>

              <Show when={isRecording()}>
                <div class="mb-6">
                  <div class="flex items-center bg-white text-dark px-4 py-2 rounded-lg inline-flex">
                    <div class="w-2 h-2 bg-dark rounded-full animate-pulse mr-2"></div>
                    <span class="text-xs font-medium">Recording</span>
                  </div>
                </div>
              </Show>

              <div class="mb-6">
                <h3 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3">
                  Push-to-Talk
                </h3>
                <div class="flex items-center justify-between p-4 bg-dark-secondary rounded-xl">
                  <div class="min-w-0">
                    <p class="font-mono text-sm font-medium text-white truncate">
                      {currentHotkey() ? (
                        modifierOptions.find(
                          (opt) => opt.value === currentHotkey()
                        )?.label
                      ) : (
                        <span class="text-gray-500">Not set</span>
                      )}
                    </p>
                  </div>
                  <button
                    onClick={() => setShowHotkeySelector(true)}
                    class="btn-secondary flex-shrink-0 ml-4"
                  >
                    Change
                  </button>
                </div>
              </div>

              <Show when={showHotkeySelector()}>
                <HotkeySelector
                  currentHotkey={currentHotkey()}
                  onHotkeyChange={handleHotkeyChange}
                  onCancel={handleHotkeyCancel}
                />
              </Show>

              <div class="mb-6">
                <h3 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3">
                  Microphone
                </h3>
                <select
                  value={selectedDevice()}
                  onChange={(e) => handleDeviceChange(e.currentTarget.value)}
                  class="select-minimal"
                >
                  <For each={audioDevices()}>
                    {(device) => (
                      <option value={device.id}>
                        {device.name} {device.is_default ? "(Default)" : ""}
                      </option>
                    )}
                  </For>
                </select>
              </div>

              <div class="mb-6">
                <h3 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3">
                  AI Model
                </h3>
                <div class="p-4 bg-dark-secondary rounded-xl">
                  <Show
                    when={!modelDownloaded()}
                    fallback={
                      <div>
                        <div class="flex items-center justify-between mb-3">
                          <div>
                            <p class="text-sm font-medium text-white">
                              {availableModels().find(
                                (m) => m.id === selectedModel()
                              )?.name || "Whisper"}
                            </p>
                            <p class="text-xs text-gray-500">
                              Ready for transcription
                            </p>
                          </div>
                          <div class="flex items-center text-xs text-gray-400">
                            <svg
                              class="w-3 h-3 mr-1 text-gray-400"
                              fill="none"
                              stroke="currentColor"
                              viewBox="0 0 24 24"
                            >
                              <path
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                stroke-width="2"
                                d="M5 13l4 4L19 7"
                              ></path>
                            </svg>
                            Downloaded
                          </div>
                        </div>
                        <button
                          onClick={() => setShowModelSelector(true)}
                          class="text-xs text-gray-400 hover:text-white transition-colors"
                        >
                          Change model
                        </button>
                      </div>
                    }
                  >
                    <Show
                      when={!isDownloading()}
                      fallback={
                        <div>
                          <div class="flex items-center justify-between mb-3">
                            <div>
                              <p class="text-sm font-medium text-white">
                                {availableModels().find(
                                  (m) => m.id === selectedModel()
                                )?.name || "Whisper"}
                              </p>
                              <p class="text-xs text-gray-500">
                                Downloading model...
                              </p>
                            </div>
                            <p class="text-xs text-gray-500">
                              {downloadProgress().toFixed(0)}%
                            </p>
                          </div>
                          <div class="mb-2">
                            <div class="h-1 bg-dark-tertiary rounded-full overflow-hidden">
                              <div
                                class="h-full bg-white transition-all duration-300"
                                style={{
                                  width: `${
                                    downloadProgress() > 0
                                      ? Math.max(
                                          2,
                                          Math.min(100, downloadProgress())
                                        )
                                      : 0
                                  }%`,
                                }}
                              ></div>
                            </div>
                          </div>

                          <div class="flex justify-between text-xs text-gray-500">
                            <span>{formatBytes(downloadedBytes())}</span>
                            <span>{formatBytes(totalBytes())}</span>
                          </div>
                        </div>
                      }
                    >
                      <div>
                        <div class="flex items-center justify-between mb-3">
                          <div>
                            <p class="text-sm font-medium text-white">
                              {availableModels().find(
                                (m) => m.id === selectedModel()
                              )?.name || "Select a model"}
                            </p>
                            <p class="text-xs text-gray-500">
                              {availableModels().find(
                                (m) => m.id === selectedModel()
                              )?.description || "Local AI transcription"}
                            </p>
                          </div>
                          <Show when={selectedModel()}>
                            <button
                              onClick={handleModelDownload}
                              class="btn-secondary"
                            >
                              Download
                            </button>
                          </Show>
                        </div>
                        <button
                          onClick={() => setShowModelSelector(true)}
                          class="text-xs text-gray-400 hover:text-white transition-colors"
                        >
                          Change model
                        </button>
                      </div>
                    </Show>
                  </Show>
                </div>
              </div>

              <Show when={showModelSelector()}>
                <div class="fixed inset-0 bg-black bg-opacity-75 flex items-center justify-center z-50">
                  <div class="bg-dark-secondary rounded-2xl p-6 max-w-md w-full mx-4 shadow-xl">
                    <h3 class="text-lg font-semibold text-white mb-4">
                      Select AI Model
                    </h3>
                    <div class="space-y-2 mb-6">
                      <For each={availableModels()}>
                        {(model) => {
                          const isRecommended =
                            model.id === "large-v3-turbo-q8_0";
                          const isDownloaded = downloadedModels().includes(
                            model.id
                          );
                          const isSelected = model.id === selectedModel();
                          return (
                            <button
                              onClick={async () => {
                                setSelectedModel(model.id);
                                const result = await commands.setSelectedModel(
                                  model.id
                                );
                                if (result.status === "ok") {
                                  setModelDownloaded(
                                    downloadedModels().includes(model.id)
                                  );
                                  setShowModelSelector(false);
                                }
                              }}
                              class={`w-full text-left p-4 rounded-lg border transition-all ${
                                isSelected
                                  ? "border-gray-600 bg-dark-tertiary"
                                  : "border-gray-700 hover:border-gray-600"
                              }`}
                            >
                              <div class="flex items-center justify-between">
                                <div>
                                  <p class="font-medium text-white">
                                    {model.name}
                                    {isRecommended && (
                                      <span class="ml-2 text-yellow-500">
                                        ⭐
                                      </span>
                                    )}
                                  </p>
                                  <p class="text-xs text-gray-500 mt-0.5">
                                    {model.size_mb}MB • {model.description}
                                  </p>
                                </div>
                                <Show when={isDownloaded}>
                                  <svg
                                    class="w-4 h-4 text-green-400"
                                    fill="none"
                                    stroke="currentColor"
                                    viewBox="0 0 24 24"
                                  >
                                    <path
                                      stroke-linecap="round"
                                      stroke-linejoin="round"
                                      stroke-width="2"
                                      d="M5 13l4 4L19 7"
                                    ></path>
                                  </svg>
                                </Show>
                              </div>
                            </button>
                          );
                        }}
                      </For>
                    </div>
                    <div class="flex justify-end">
                      <button
                        onClick={() => setShowModelSelector(false)}
                        class="btn-secondary"
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                </div>
              </Show>

              <div class="">
                <h3 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3">
                  Permissions
                </h3>
                <div class="space-y-2">
                  <div class="flex items-center justify-between p-4 bg-dark-secondary rounded-xl">
                    <div class="min-w-0">
                      <p class="text-sm font-medium text-white">Microphone</p>
                      <p class="text-xs text-gray-500">Voice recording</p>
                    </div>
                    <Show
                      when={micPermission()}
                      fallback={
                        <button
                          onClick={requestMicrophonePermission}
                          class="btn-text flex-shrink-0 ml-4"
                        >
                          Enable
                        </button>
                      }
                    >
                      <div class="flex items-center text-xs text-gray-400 flex-shrink-0 ml-4">
                        <svg
                          class="w-3 h-3 mr-1 text-gray-400"
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M5 13l4 4L19 7"
                          ></path>
                        </svg>
                        Enabled
                      </div>
                    </Show>
                  </div>

                  <div class="flex items-center justify-between p-4 bg-dark-secondary rounded-xl">
                    <div class="min-w-0">
                      <p class="text-sm font-medium text-white">
                        Accessibility
                      </p>
                      <p class="text-xs text-gray-500">Text insertion</p>
                    </div>
                    <Show
                      when={accessibilityPermission()}
                      fallback={
                        <button
                          onClick={requestAccessibilityPermission}
                          class="btn-text flex-shrink-0 ml-4"
                        >
                          Enable
                        </button>
                      }
                    >
                      <div class="flex items-center text-xs text-gray-400 flex-shrink-0 ml-4">
                        <svg
                          class="w-3 h-3 mr-1 text-gray-400"
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M5 13l4 4L19 7"
                          ></path>
                        </svg>
                        Enabled
                      </div>
                    </Show>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
