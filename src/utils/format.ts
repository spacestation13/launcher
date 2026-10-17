import i18n from "../i18n";

export const formatDuration = (deciseconds: number | undefined): string => {
  if (!deciseconds) return "--:--:--";
  const totalSeconds = Math.floor(deciseconds / 10);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return `${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
};

export const formatRegion = (region: string): string => {
  const key = `regions.${region}`;
  if (i18n.exists(key)) return i18n.t(key as "regions.antarctica");
  return region
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
};
