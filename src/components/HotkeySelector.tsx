import { createSignal, onMount } from "solid-js";

interface HotkeySelectorProps {
  currentHotkey: string | null;
  onHotkeyChange: (hotkey: string) => void;
  onCancel: () => void;
}

function HotkeySelector(props: HotkeySelectorProps) {
  const [selectedModifier, setSelectedModifier] = createSignal<string>(
    props.currentHotkey || ""
  );

  const modifierOptions = [
    { value: "rightOption", label: "Right Option (⌥)" },
    { value: "leftOption", label: "Left Option (⌥)" },
    { value: "leftControl", label: "Left Control (⌃)" },
    { value: "rightControl", label: "Right Control (⌃)" },
    { value: "fn", label: "Fn" },
    { value: "rightCommand", label: "Right Command (⌘)" },
    { value: "rightShift", label: "Right Shift (⇧)" },
  ];

  const handleSave = () => {
    const selected = selectedModifier();
    if (selected) {
      props.onHotkeyChange(selected);
    }
  };

  const cancel = () => {
    setSelectedModifier("");
    props.onCancel();
  };

  onMount(() => {
    if (props.currentHotkey) {
      setSelectedModifier(props.currentHotkey);
    }
  });

  return (
    <div class="fixed inset-0 bg-black/10 backdrop-blur-sm flex items-center justify-center z-50">
      <div class="bg-white rounded-2xl p-6 max-w-sm w-full mx-4 shadow-xl">
        <h3 class="text-lg font-semibold text-black mb-4">
          Set Push-to-Talk Key
        </h3>

        <div class="mb-4">
          <p class="text-xs text-gray-400 mb-1">Current</p>
          <div class="text-base font-medium text-black">
            {props.currentHotkey ? (
              modifierOptions.find((opt) => opt.value === props.currentHotkey)
                ?.label
            ) : (
              <span class="text-gray-400">None</span>
            )}
          </div>
        </div>


        <div class="mb-6">
          <p class="text-xs text-gray-400 mb-2">Select new key</p>
          <select
            value={selectedModifier()}
            onChange={(e) => setSelectedModifier(e.target.value)}
            class="w-full px-4 py-3 rounded-xl border border-gray-300 text-black bg-white focus:ring-2 focus:ring-black/10 focus:border-gray-400 transition-all duration-200 cursor-pointer appearance-none text-sm"
            style={{
              "background-image": "url(\"data:image/svg+xml,%3csvg xmlns='http://www.w3.org/2000/svg' fill='none' viewBox='0 0 20 20'%3e%3cpath stroke='%236b7280' stroke-linecap='round' stroke-linejoin='round' stroke-width='1.5' d='M6 8l4 4 4-4'/%3e%3c/svg%3e\")",
              "background-position": "right 1rem center",
              "background-repeat": "no-repeat",
              "background-size": "1.5em 1.5em",
              "padding-right": "3rem"
            }}
          >
            <option value="">Choose...</option>
            {modifierOptions.map((option) => (
              <option value={option.value}>{option.label}</option>
            ))}
          </select>
          <p class="text-xs text-gray-400 mt-2">
            Tip: Fn key works great for push-to-talk
          </p>
        </div>

        <div class="flex gap-2">
          <button
            onClick={cancel}
            class="flex-1 px-4 py-2 rounded-lg bg-gray-100 text-gray-700 font-medium text-sm hover:bg-gray-200 transition-colors"
          >
            Cancel
          </button>

          <button
            onClick={handleSave}
            disabled={!selectedModifier()}
            class="flex-1 px-4 py-2 rounded-lg bg-black text-white font-medium text-sm hover:bg-gray-900 transition-colors disabled:bg-gray-200 disabled:text-gray-400 disabled:cursor-not-allowed"
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}

export default HotkeySelector;
