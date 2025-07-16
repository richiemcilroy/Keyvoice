import { createSignal, onMount, For, Show, createMemo } from "solid-js";
import { commands, type Transcript } from "../bindings";

interface TimelineProps {
  transcripts: Transcript[];
  onCopyText: (text: string) => void;
  onDeleteTranscript: (id: string) => void;
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
    <div class="h-full overflow-y-auto px-8 pb-20">
      <For each={groupedTranscripts()}>
        {([groupKey, transcripts]) => (
          <div class="mb-8">
            <h3 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-4 sticky z-40 top-0 py-2">
              <span class="bg-dark-secondary px-3 py-1 rounded-full">
                {groupKey}
              </span>
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
                      <div class="flex-shrink-0 text-xs text-gray-500 w-20">
                        {formatTime(transcript.timestamp)}
                      </div>
                      <div class="flex-1">
                        <p class="text-sm text-gray-300 leading-relaxed">
                          {transcript.text}
                        </p>
                        <Show when={transcript.text === ""}>
                          <p class="text-sm text-gray-500 italic">
                            Audio is silent.
                          </p>
                        </Show>
                      </div>
                    </div>
                    <Show when={hoveredId() === transcript.id}>
                      <div class="absolute right-0 top-0 flex gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                        <Show when={transcript.text}>
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              handleCopy(transcript.text);
                            }}
                            class="px-3 py-1 bg-gray-800 text-gray-300 text-xs font-medium rounded hover:bg-gray-700 transition-colors"
                          >
                            Copy
                          </button>
                        </Show>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            if (confirm("Delete this transcript?")) {
                              props.onDeleteTranscript(transcript.id);
                            }
                          }}
                          class="px-3 py-1 bg-gray-800 text-gray-300 text-xs font-medium rounded hover:bg-red-900 hover:text-red-200 transition-colors"
                        >
                          Delete
                        </button>
                      </div>
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
          <p class="text-gray-500 text-sm">No transcriptions yet</p>
          <p class="text-gray-600 text-xs mt-1">
            Press your hotkey to start recording
          </p>
        </div>
      </Show>
    </div>
  );
}
