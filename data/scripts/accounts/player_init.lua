local PlayerInit = {}

PlayerInit.context = {}

function PlayerInit.player_init(ctx)
  local _ = ctx
  return {
    ship_bundle_id = "ship.corvette",
  }
end

return PlayerInit
