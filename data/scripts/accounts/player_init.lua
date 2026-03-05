local PlayerInit = {}

PlayerInit.context = {}

function PlayerInit.player_init(ctx)
  local _ = ctx
  return {
    starter_bundle_id = "starter_corvette",
  }
end

return PlayerInit
