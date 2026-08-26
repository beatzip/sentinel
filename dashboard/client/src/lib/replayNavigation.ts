export type ReplayNavigationPlayer = { steam_id: number; name: string; duck_amount?: number | null };
export type ReplayNavigationFrame = { players: ReplayNavigationPlayer[] };

export function matchingPlayerIds(players: ReplayNavigationPlayer[], query: string) {
  const normalized = query.trim().toLocaleLowerCase();
  return players
    .filter((player) => !normalized || player.name.toLocaleLowerCase().includes(normalized))
    .map((player) => player.steam_id);
}

export function nextObservedCrouchFrame(
  frames: ReplayNavigationFrame[],
  playerId: number | null,
  currentIndex: number,
  direction: -1 | 1,
) {
  if (playerId === null || !frames.length) return null;
  for (let offset = 1; offset <= frames.length; offset += 1) {
    const candidate = (currentIndex + direction * offset + frames.length) % frames.length;
    const player = frames[candidate]?.players.find((item) => item.steam_id === playerId);
    if ((player?.duck_amount ?? 0) >= 0.5) return candidate;
  }
  return null;
}
