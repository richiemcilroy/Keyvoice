import { render } from "solid-js/web";
import { createSignal, onMount, onCleanup, For } from "solid-js";
import { events } from "./bindings";
import "./app.css";
import "./bubble.css";

function BubbleApp() {
  const [audioLevels, setAudioLevels] = createSignal<number[]>(
    Array(8).fill(0)
  );
  const [isAnimatingIn, setIsAnimatingIn] = createSignal(true);
  const [isAnimatingOut, setIsAnimatingOut] = createSignal(false);

  let animationFrame: number | null = null;
  let levelDecayTimer: number | null = null;

  onMount(async () => {
    setTimeout(() => setIsAnimatingIn(false), 300);

    const audioLevelUnlisten = await events.audioLevelUpdate.listen((event) => {
      updateAudioLevels(event.payload.level);
    });

    const recordingStateUnlisten = await events.recordingStateChanged.listen(
      (event) => {
        if (event.payload.is_recording) {
          setIsAnimatingOut(false);
          setIsAnimatingIn(true);
          setTimeout(() => setIsAnimatingIn(false), 300);
        } else {
          setIsAnimatingOut(true);
          setTimeout(() => setIsAnimatingOut(false), 300);
        }
      }
    );

    startSimulatedAudioLevels();

    onCleanup(() => {
      audioLevelUnlisten();
      recordingStateUnlisten();
      if (animationFrame) cancelAnimationFrame(animationFrame);
      if (levelDecayTimer) clearInterval(levelDecayTimer);
    });
  });

  const updateAudioLevels = (level: number) => {
    const newLevels = audioLevels().map((_, index) => {
      const variation = Math.sin(Date.now() / 100 + index) * 0.3;
      return Math.max(0, Math.min(1, level + variation));
    });
    setAudioLevels(newLevels);
  };

  const startSimulatedAudioLevels = () => {
    const animate = () => {
      const time = Date.now() / 1000;
      const newLevels = audioLevels().map((_, index) => {
        const frequency = 0.5 + index * 0.1;
        const amplitude = 0.3 + Math.sin(time * 2 + index) * 0.2;
        const value = amplitude * Math.sin(time * frequency * Math.PI);
        return Math.abs(value);
      });
      setAudioLevels(newLevels);
      animationFrame = requestAnimationFrame(animate);
    };
    animate();
  };

  return (
    <div
      class={`flex items-center justify-center bg-black/90 rounded-[17.5px] shadow shadow-black/25 origin-bottom transition-all duration-300 ease-[cubic-bezier(0.34,1.56,0.64,1)] ${
        isAnimatingIn() || isAnimatingOut()
          ? "w-10 h-[10px] opacity-0 scale-[0.8] translate-y-[10px]"
          : "w-[70px] h-[35px] opacity-100 scale-100"
      }`}
    >
      <div class="flex items-center gap-[2px] h-full px-[10px]">
        <For each={audioLevels()}>
          {(level) => (
            <div
              class="w-[2px] bg-white rounded transition-all duration-100 ease-out"
              style={{
                height: `${10 + level * 15}px`,
                opacity: 0.6 + level * 0.4,
              }}
            />
          )}
        </For>
      </div>
    </div>
  );
}

const root = document.getElementById("bubble-root");

if (root) {
  render(() => <BubbleApp />, root);
}
