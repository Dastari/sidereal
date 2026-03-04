local DynamicBundle = {}

DynamicBundle.context = {}

function DynamicBundle.build_graph_records(ctx)
  return {
    {
      entity_id = ctx.player_entity_id,
      labels = { "Entity", "Player" },
      properties = {},
      components = {
        { component_id = ctx.player_entity_id .. ":display_name", component_kind = "display_name", properties = "Dynamic Player" },
        { component_id = ctx.player_entity_id .. ":player_tag", component_kind = "player_tag", properties = {} },
        { component_id = ctx.player_entity_id .. ":account_id", component_kind = "account_id", properties = ctx.account_id },
        { component_id = ctx.player_entity_id .. ":entity_labels", component_kind = "entity_labels", properties = { "Player" } },
      },
    },
  }
end

return DynamicBundle
