local NewAccount = {}

NewAccount.context = {}

function NewAccount.on_new_account(ctx)
  local _ = ctx
  return {
    starter_bundle_id = "starter_corvette",
  }
end

return NewAccount
