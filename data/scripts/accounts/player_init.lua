local PlayerInit = {}

PlayerInit.context = {}

function PlayerInit.player_init(ctx)
  local _ = ctx
  return {
    player_bundle_id = "player.default",
    controlled_bundle_id = "ship.corvette",
  }
end

return PlayerInit
