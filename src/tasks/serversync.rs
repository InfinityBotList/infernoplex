pub async fn server_sync(ctx: &serenity::all::Context) -> Result<(), crate::Error> {
    // Loop over every single guild we currently have
    for _guild in ctx.cache.guilds() {}
    Ok(())
}
