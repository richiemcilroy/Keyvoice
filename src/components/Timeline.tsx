import { createSignal, onMount, For, Show, createMemo } from "solid-js";
import { commands, type Transcript } from "../bindings";

interface TimelineProps {
  transcripts: Transcript[];
  onCopyText: (text: string) => void;
}

export default function Timeline(props: TimelineProps) {
  const [hoveredId, setHoveredId] = createSignal<string | null>(null);

  const groupedTranscripts = createMemo(() => {
    const groups: Record<string, Transcript[]> = {};
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);

    props.transcripts.forEach((transcript) => {
      const date = new Date(transcript.timestamp);
      let groupKey: string;

      if (date >= today) {
        groupKey = "Today";
      } else if (date >= yesterday) {
        groupKey = "Yesterday";
      } else {
        groupKey = date.toLocaleDateString("en-US", {
          weekday: "long",
          year: "numeric",
          month: "long",
          day: "numeric",
        });
      }

      if (!groups[groupKey]) {
        groups[groupKey] = [];
      }
      groups[groupKey].push(transcript);
    });

    return Object.entries(groups).sort((a, b) => {
      if (a[0] === "Today") return -1;
      if (b[0] === "Today") return 1;
      if (a[0] === "Yesterday") return -1;
      if (b[0] === "Yesterday") return 1;
      return new Date(b[0]).getTime() - new Date(a[0]).getTime();
    });
  });

  const formatTime = (timestamp: number) => {
    return new Date(timestamp).toLocaleTimeString("en-US", {
      hour: "numeric",
      minute: "2-digit",
      hour12: true,
    });
  };

  const handleCopy = async (text: string) => {
    await navigator.clipboard.writeText(text);
    props.onCopyText(text);
  };

  return (
    <div class="h-full overflow-y-auto">
      <For each={groupedTranscripts()}>
        {([groupKey, transcripts]) => (
          <div class="mb-8">
            <h3 class="text-xs font-medium text-gray-400 uppercase tracking-wider mb-4 sticky top-0 bg-white py-2">
              {groupKey}
            </h3>
            <div class="space-y-3">
              <For each={transcripts}>
                {(transcript) => (
                  <div
                    class="relative group"
                    onMouseEnter={() => setHoveredId(transcript.id)}
                    onMouseLeave={() => setHoveredId(null)}
                  >
                    <div class="flex gap-4">
                      <div class="flex-shrink-0 text-xs text-gray-400 w-20">
                        {formatTime(transcript.timestamp)}
                      </div>
                      <div class="flex-1">
                        <p class="text-sm text-gray-700 leading-relaxed">
                          {transcript.text}
                        </p>
                        <Show when={transcript.text === ""}>
                          <p class="text-sm text-gray-400 italic">
                            Audio is silent.
                          </p>
                        </Show>
                      </div>
                    </div>
                    <Show when={hoveredId() === transcript.id && transcript.text}>
                      <button
                        onClick={() => handleCopy(transcript.text)}
                        class="absolute right-0 top-0 p-1 bg-white border border-gray-200 rounded shadow-sm opacity-0 group-hover:opacity-100 transition-opacity"
                        title="Copy text"
                      >
                        <svg
                          class="w-4 h-4 text-gray-600"
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                          />
                        </svg>
                      </button>
                    </Show>
                  </div>
                )}
              </For>
            </div>
          </div>
        )}
      </For>
      <Show when={props.transcripts.length === 0}>
        <div class="text-center py-12">
          <p class="text-gray-400 text-sm">No transcriptions yet</p>
          <p class="text-gray-300 text-xs mt-1">
            Press your hotkey to start recording
          </p>
        </div>
      </Show>
    </div>
  );
}