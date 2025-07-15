import { createSignal, onMount, For, Show, onCleanup } from "solid-js";
import { commands, events } from "./bindings";
import HotkeySelector from "./components/HotkeySelector";

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

  const modifierOptions = [
    { value: "rightOption", label: "Right Option (⌥)" },
    { value: "leftOption", label: "Left Option (⌥)" },
    { value: "leftControl", label: "Left Control (⌃)" },
    { value: "rightControl", label: "Right Control (⌃)" },
    { value: "fn", label: "Fn" },
    { value: "rightCommand", label: "Right Command (⌘)" },
    { value: "rightShift", label: "Right Shift (⇧)" },
  ];

  onMount(async () => {
    // Check if model is downloaded
    const modelResult = await commands.checkModelDownloaded();
    if (modelResult.status === "ok") {
      setModelDownloaded(modelResult.data);
    }

    // Listen for download events
    const progressUnlisten = await events.modelDownloadProgress.listen(
      (event) => {
        setDownloadProgress(event.payload.progress);
        setDownloadedBytes(event.payload.downloaded_bytes);
        setTotalBytes(event.payload.total_bytes);
      }
    );

    const completeUnlisten = await events.modelDownloadComplete.listen(
      (event) => {
        setModelDownloaded(event.payload.success);
        setIsDownloading(false);
        if (!event.payload.success && event.payload.error) {
          console.error("Model download failed:", event.payload.error);
        }
      }
    );

    // Load initial data
    try {
      const devicesResult = await commands.getAudioDevices();
      if (devicesResult.status === "ok") {
        setAudioDevices(devicesResult.data);

        // Check for saved device selection first
        const currentDeviceResult = await commands.getCurrentDevice();
        if (currentDeviceResult.status === "ok" && currentDeviceResult.data) {
          setSelectedDevice(currentDeviceResult.data);
        } else {
          // Select default device if no saved selection
          const defaultDevice = devicesResult.data.find((d) => d.is_default);
          if (defaultDevice) {
            setSelectedDevice(defaultDevice.id);
            // Save the default selection
            await commands.setRecordingDevice(defaultDevice.id);
          }
        }
      }

      // Get word count
      const countResult = await commands.getWordCount();
      if (countResult.status === "ok") {
        setWordCount(countResult.data);
      }

      // Check permissions
      const permissionsResult = await commands.checkPermissions();
      if (permissionsResult.status === "ok") {
        setMicPermission(permissionsResult.data.microphone.state === "Granted");
        setAccessibilityPermission(
          permissionsResult.data.accessibility.state === "Granted"
        );
      }

      // Get current hotkey and register it
      const hotkeyResult = await commands.getHotkey();
      if (hotkeyResult.status === "ok" && hotkeyResult.data) {
        setCurrentHotkey(hotkeyResult.data);
        // Register the saved hotkey with the backend
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

    // Listen for Fn key state changes
    const fnKeyUnlisten = await events.fnKeyStateChanged.listen((event) => {
      console.log("Fn key state changed:", event.payload.is_pressed);
      setFnKeyPressed(event.payload.is_pressed);
    });

    // Listen for recording state changes
    const recordingUnlisten = await events.recordingStateChanged.listen(
      (event) => {
        console.log("Recording state changed:", event.payload.is_recording);
        setIsRecording(event.payload.is_recording);
      }
    );

    // Set up periodic permission checking
    const permissionCheckInterval = setInterval(refreshPermissions, 5000); // Check every 5 seconds

    // Check permissions when window gains focus
    const handleWindowFocus = () => {
      refreshPermissions();
    };

    window.addEventListener("focus", handleWindowFocus);

    onCleanup(() => {
      fnKeyUnlisten();
      recordingUnlisten();
      progressUnlisten();
      completeUnlisten();
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
        // After requesting, check permissions again to get updated state
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
      // Open macOS accessibility settings directly
      await commands.requestAccessibilityPermission();

      // After requesting, check permissions again to get updated state
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

  return (
    <div class="min-h-screen bg-white relative overflow-hidden">
      {/* Draggable title bar area */}
      <div
        data-tauri-drag-region
        class="absolute top-0 left-0 right-0 h-16 z-50"
        style="-webkit-app-region: drag;"
      />
      <div class="p-8 pt-20">
        <div class="w-full max-w-md mx-auto">
          <h1 class="text-4xl font-bold text-black mb-2">TalkType.</h1>
          <p class="text-gray-500 text-sm mb-10">
            Your voice, transcribed instantly.
          </p>

          {/* Word Counter */}
          <div class="mb-12">
            <div class="text-5xl font-bold text-black mb-1">{wordCount()}</div>
            <p class="text-gray-400 text-xs">words transcribed</p>
          </div>

          {/* Recording Status */}
          <Show when={isRecording()}>
            <div class="fixed top-20 right-8">
              <div class="flex items-center bg-black text-white px-4 py-2 rounded-lg">
                <div class="w-2 h-2 bg-white rounded-full animate-pulse mr-2"></div>
                <span class="text-xs font-medium">Recording</span>
              </div>
            </div>
          </Show>

          {/* Hotkey Configuration */}
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

          {/* Hotkey Selector Modal */}
          <Show when={showHotkeySelector()}>
            <HotkeySelector
              currentHotkey={currentHotkey()}
              onHotkeyChange={handleHotkeyChange}
              onCancel={handleHotkeyCancel}
            />
          </Show>

          {/* Microphone Selection */}
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

          {/* Whisper Model Download */}
          <div class="mb-6">
            <h3 class="text-xs font-medium text-gray-400 uppercase tracking-wider mb-3">
              AI Model
            </h3>
            <div class="p-4 bg-gray-50 rounded-xl">
              <Show
                when={!modelDownloaded()}
                fallback={
                  <div class="flex items-center justify-between">
                    <div>
                      <p class="text-sm font-medium text-black">Whisper</p>
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
                          <p class="text-sm font-medium text-black">Whisper</p>
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
                      <p class="text-sm font-medium text-black">Whisper</p>
                      <p class="text-xs text-gray-400">
                        Local AI transcription model
                      </p>
                    </div>
                    <button onClick={handleModelDownload} class="btn-secondary">
                      Download
                    </button>
                  </div>
                </Show>
              </Show>
            </div>
          </div>

          {/* Permissions */}
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

          {/* Footer */}
          <div class="mt-12 pt-6 border-t border-gray-100">
            <p class="text-xs text-gray-300 text-center">© 2025 TalkType</p>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
