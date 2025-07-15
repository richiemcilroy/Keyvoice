import { createSignal, onMount, For, Show, onCleanup, createEffect } from "solid-js";
import { commands, events, type WhisperModelInfo, type Transcript } from "./bindings";
import HotkeySelector from "./components/HotkeySelector";
import Timeline from "./components/Timeline";

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
  const [showCopiedNotification, setShowCopiedNotification] = createSignal(false);

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

      // Load transcripts and stats from transcript store instead
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
      }
    );

    const statsUnlisten = await events.recordingStatsUpdated.listen(async (event) => {
      console.log("Recording stats updated:", event.payload);
      setRecordingStats(event.payload);
      // Reload transcripts when new recording is added
      await loadTranscripts();
    });

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

      const result = await commands.downloadWhisperModel();
      if (result.status === "error") {
        console.error("Failed to download model:", result.error);
        setIsDownloading(false);
      }
    } catch (error) {
      console.error("Failed to download model:", error);
      setIsDownloading(false);
    }
  };

  const handleCopyText = (text: string) => {
    setShowCopiedNotification(true);
    setTimeout(() => setShowCopiedNotification(false), 2000);
  };

  return (
    <div class="min-h-screen bg-white relative overflow-hidden">
      <div
        data-tauri-drag-region
        class="absolute top-0 left-0 right-0 h-16 z-50"
        style="-webkit-app-region: drag;"
      />
      
      <Show when={showCopiedNotification()}>
        <div class="fixed top-20 left-1/2 transform -translate-x-1/2 bg-black text-white px-4 py-2 rounded-lg text-sm z-50 animate-fade-in">
          Copied to clipboard
        </div>
      </Show>

      <div class="flex flex-col h-screen pt-16">
        {/* Header with title and stats */}
        <div class="px-8 py-6 border-b border-gray-100">
          <div class="max-w-6xl mx-auto">
            <div class="flex items-start justify-between">
              <div>
                <h1 class="text-4xl font-bold text-black mb-2">TalkType.</h1>
                <p class="text-gray-500 text-sm">
                  Your voice, transcribed instantly.
                </p>
              </div>
              <div class="flex gap-12">
                <div class="text-center">
                  <div class="text-4xl font-bold text-black mb-1">
                    {wordCount()}
                  </div>
                  <p class="text-gray-400 text-xs">words transcribed</p>
                </div>
                <div class="text-center">
                  <div class="text-4xl font-bold text-black mb-1">
                    {recordingStats().overall_wpm.toFixed(0)}
                  </div>
                  <p class="text-gray-400 text-xs">words per minute</p>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Main content - Timeline and Settings columns */}
        <div class="flex flex-1 overflow-hidden">
          {/* Left column - Timeline */}
          <div class="w-1/2 border-r border-gray-100 p-8 overflow-hidden">
            <h2 class="text-xl font-semibold text-black mb-6">Timeline</h2>
            <Timeline transcripts={transcripts()} onCopyText={handleCopyText} />
          </div>

          {/* Right column - Settings */}
          <div class="w-1/2 p-8 overflow-y-auto">
            <div class="max-w-md">
              <h2 class="text-xl font-semibold text-black mb-6">Settings</h2>

              <Show when={isRecording()}>
                <div class="mb-6">
                  <div class="flex items-center bg-black text-white px-4 py-2 rounded-lg inline-flex">
                    <div class="w-2 h-2 bg-white rounded-full animate-pulse mr-2"></div>
                    <span class="text-xs font-medium">Recording</span>
                  </div>
                </div>
              </Show>

          <div class="mb-6">
            <h3 class="text-xs font-medium text-gray-400 uppercase tracking-wider mb-3">
              Push-to-Talk
            </h3>
            <div class="flex items-center justify-between p-4 bg-gray-50 rounded-xl">
              <div class="min-w-0">
                <p class="font-mono text-base text-black truncate">
                  {currentHotkey() ? (
                    modifierOptions.find((opt) => opt.value === currentHotkey())
                      ?.label
                  ) : (
                    <span class="text-gray-400">Not set</span>
                  )}
                </p>
                <p class="text-xs text-gray-400 mt-0.5">Hold to record</p>
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
            <h3 class="text-xs font-medium text-gray-400 uppercase tracking-wider mb-3">
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
            <h3 class="text-xs font-medium text-gray-400 uppercase tracking-wider mb-3">
              AI Model
            </h3>
            <Show when={availableModels().length > 0}>
              <select
                value={selectedModel() || ""}
                onChange={async (e) => {
                  const modelId = e.currentTarget.value;
                  if (modelId) {
                    setSelectedModel(modelId);
                    const result = await commands.setSelectedModel(modelId);
                    if (result.status === "ok") {
                      setModelDownloaded(downloadedModels().includes(modelId));
                    }
                  }
                }}
                class="select-minimal mb-3"
              >
                <For each={availableModels()}>
                  {(model) => {
                    const isRecommended = model.id === "large-v3-turbo-q8_0";
                    const isDownloaded = downloadedModels().includes(model.id);
                    return (
                      <option value={model.id}>
                        {model.name} ({model.size_mb}MB)
                        {isRecommended && " ⭐ Recommended"}
                        {isDownloaded && " ✓"}
                      </option>
                    );
                  }}
                </For>
              </select>
            </Show>
            <div class="p-4 bg-gray-50 rounded-xl">
              <Show
                when={!modelDownloaded()}
                fallback={
                  <div class="flex items-center justify-between">
                    <div>
                      <p class="text-sm font-medium text-black">
                        {availableModels().find((m) => m.id === selectedModel())
                          ?.name || "Whisper"}
                      </p>
                      <p class="text-xs text-gray-400">
                        Ready for transcription
                      </p>
                    </div>
                    <div class="flex items-center text-xs text-gray-400">
                      <svg
                        class="w-3 h-3 mr-1"
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
                }
              >
                <Show
                  when={!isDownloading()}
                  fallback={
                    <div>
                      <div class="flex items-center justify-between mb-3">
                        <div>
                          <p class="text-sm font-medium text-black">
                            {availableModels().find(
                              (m) => m.id === selectedModel()
                            )?.name || "Whisper"}
                          </p>
                          <p class="text-xs text-gray-400">
                            Downloading model...
                          </p>
                        </div>
                        <p class="text-xs text-gray-400">
                          {downloadProgress().toFixed(0)}%
                        </p>
                      </div>
                      <div class="mb-2">
                        <div class="h-1 bg-gray-200 rounded-full overflow-hidden">
                          <div
                            class="h-full bg-black transition-all duration-300"
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

                      <div class="flex justify-between text-xs text-gray-400">
                        <span>{formatBytes(downloadedBytes())}</span>
                        <span>{formatBytes(totalBytes())}</span>
                      </div>
                    </div>
                  }
                >
                  <div class="flex items-center justify-between">
                    <div>
                      <p class="text-sm font-medium text-black">
                        {availableModels().find((m) => m.id === selectedModel())
                          ?.name || "Select a model"}
                      </p>
                      <p class="text-xs text-gray-400">
                        {availableModels().find((m) => m.id === selectedModel())
                          ?.description || "Local AI transcription"}
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
                </Show>
              </Show>
            </div>
          </div>

          <div class="">
            <h3 class="text-xs font-medium text-gray-400 uppercase tracking-wider mb-3">
              Permissions
            </h3>
            <div class="space-y-2">
              <div class="flex items-center justify-between p-4 bg-gray-50 rounded-xl">
                <div class="min-w-0">
                  <p class="text-sm font-medium text-black">Microphone</p>
                  <p class="text-xs text-gray-400">Voice recording</p>
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
                      class="w-3 h-3 mr-1"
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

              <div class="flex items-center justify-between p-4 bg-gray-50 rounded-xl">
                <div class="min-w-0">
                  <p class="text-sm font-medium text-black">Accessibility</p>
                  <p class="text-xs text-gray-400">Text insertion</p>
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
                      class="w-3 h-3 mr-1"
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
