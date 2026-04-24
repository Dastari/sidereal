export type GenesisPlanetEntry = {
  planetId: string
  scriptPath: string
  displayName: string
  bodyKind: number | null
  planetType: number | null
  seed: number | null
  spawnEnabled: boolean
  tags: Array<string>
  hasDraft: boolean
}

export type GenesisPlanetCatalog = {
  entries: Array<GenesisPlanetEntry>
  registryHasDraft: boolean
}
